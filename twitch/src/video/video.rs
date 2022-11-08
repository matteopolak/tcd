use std::{collections::HashMap, pin::Pin};

use async_stream::stream;
use chrono::{DateTime, FixedOffset};
use futures::{Stream, StreamExt};
use prisma_client_rust::QueryError;
use serde::Deserialize;
use serde_json::json;

use crate::{
	gql::gql::{GqlResponse, GqlTrackedUser, GqlVideoContent},
	prisma::{self, PrismaClient},
};

#[derive(Debug)]
pub struct Video {
	pub id: i64,
	pub author: String,
	pub author_id: i64,
	pub cursor: Option<String>,
	pub created_at: DateTime<FixedOffset>,
}

#[derive(Deserialize, Debug)]
pub struct GqlTrackedUserResponse {
	pub user: GqlTrackedUser,
}

#[derive(Deserialize, Debug)]
pub struct GqlVideoContentResponse {
	pub video: Option<GqlVideoContent>,
}

impl Video {
	pub fn new(
		id: i64,
		author: String,
		author_id: i64,
		cursor: Option<String>,
		created_at: DateTime<FixedOffset>,
	) -> Self {
		Self {
			id,
			author,
			author_id,
			cursor,
			created_at,
		}
	}

	pub async fn save(self, client: &PrismaClient) -> Result<(), QueryError> {
		client
			.video()
			.create(
				self.id,
				prisma::user::UniqueWhereParam::IdEquals(self.author_id),
				self.created_at,
				vec![],
			)
			.exec()
			.await
			.unwrap();

		Ok(())
	}

	pub fn get_comments(&self) -> CommentIterator {
		CommentIterator::new(self.author_id, self.id, self.created_at)
	}

	async fn get_next_chunk(&self) -> Option<Vec<Self>> {
		let cursor = match self.cursor {
			Some(ref cursor) => cursor,
			None => return None,
		};

		let client = reqwest::Client::new();
		let videos = client
			.post("https://gql.twitch.tv/gql")
			.header("Client-ID", std::env::var("CLIENT_ID").unwrap())
			.json(&json!({
				"operationName": "FilterableVideoTower_Videos",
				"variables": {
					"limit": 30,
					"channelOwnerLogin": self.author,
					"broadcastType": "ARCHIVE",
					"videoSort": "TIME",
					"cursor": cursor,
				},
				"extensions": {
					"persistedQuery": {
						"version": 1,
						"sha256Hash":
							"a937f1d22e269e39a03b509f65a7490f9fc247d7f83d6ac1421523e3b68042cb",
					},
				},
			}))
			.send()
			.await
			.unwrap();

		let videos: GqlResponse<GqlTrackedUserResponse> = videos.json().await.unwrap();

		videos.data.user.videos.and_then(|videos| {
			Some(
				videos
					.edges
					.into_iter()
					.map(|edge| {
						Self::new(
							edge.node.id,
							self.author.clone(),
							self.author_id,
							edge.cursor,
							edge.node.created_at,
						)
					})
					.collect(),
			)
		})
	}
}

pub struct CommentIterator {
	pub author_id: i64,
	pub video_id: i64,
	pub created_at: DateTime<FixedOffset>,
}

impl CommentIterator {
	pub fn new(author_id: i64, video_id: i64, created_at: DateTime<FixedOffset>) -> Self {
		Self {
			author_id,
			video_id,
			created_at,
		}
	}

	pub async fn download_all(mut self, client: &PrismaClient) -> Result<(), QueryError> {
		let video_id = self.video_id;
		let mut chunks = self.iter();

		while let Some(chunk) = chunks.next().await {
			let mut comments: Vec<(
				String,
				i64,
				i64,
				::prisma_client_rust::chrono::DateTime<::prisma_client_rust::chrono::FixedOffset>,
				Vec<prisma::comment::SetParam>,
			)> = vec![];
			let mut fragments: Vec<(i32, String, String, Vec<prisma::comment_fragment::SetParam>)> =
				vec![];
			let mut users: HashMap<i64, (i64, String, Vec<prisma::user::SetParam>)> =
				HashMap::new();

			for comment in chunk.comments.edges {
				let commenter = match comment.node.commenter {
					Some(commenter) => commenter,
					None => continue,
				};

				let comment_id = comment.node.id.clone();

				users.entry(commenter.id).or_insert_with(|| {
					prisma::user::create_unchecked(commenter.id, commenter.username, vec![])
				});

				fragments.extend(comment.node.message.fragments.into_iter().enumerate().map(
					|(index, fragment)| {
						prisma::comment_fragment::create_unchecked(
							index as i32,
							comment_id.clone(),
							fragment.text,
							vec![prisma::comment_fragment::emote::set(
								fragment.emote.and_then(|e| Some(e.emote_id)),
							)],
						)
					},
				));

				comments.push(prisma::comment::create_unchecked(
					comment.node.id,
					commenter.id,
					video_id,
					comment.node.created_at,
					vec![],
				));
			}

			let users_len = users.len();
			let comments_len = comments.len();
			let fragments_len = fragments.len();

			client
				.user()
				.create_many(users.into_values().into_iter().collect())
				.skip_duplicates()
				.exec()
				.await
				.unwrap();

			client
				.comment()
				.create_many(comments)
				.skip_duplicates()
				.exec()
				.await
				.unwrap();

			client
				.comment_fragment()
				.create_many(fragments)
				.skip_duplicates()
				.exec()
				.await
				.unwrap();

			println!(
				"[{}] Added {} users, {} comments, {} fragments",
				video_id, users_len, comments_len, fragments_len
			);
		}

		Ok(())
	}

	pub fn iter(&mut self) -> Pin<Box<impl Stream<Item = GqlVideoContent> + '_>> {
		Box::pin(stream! {
			let mut next_chunk = self.get_next_chunk().await;
			let mut payload = json!(
				{
					"operationName": "VideoCommentsByOffsetOrCursor",
					"variables": {
						"videoID": self.video_id.to_string(),
						"cursor": "",
					},
					"extensions": {
						"persistedQuery": {
							"version": 1,
							"sha256Hash":
								"b70a3591ff0f4e0313d126c6a1502d79a1c02baebb288227c582044aa76adf6a",
						},
					},
				}
			);

			loop {
				if let Some(chunk) = next_chunk {
					let video = match chunk.video {
						Some(video) => video,
						None => break,
					};

					let last = match video.comments.edges.last() {
						Some(last) => last,
						None => break,
					};

					let cursor = match last.cursor.clone() {
						Some(cursor) => cursor,
						None => break,
					};

					yield video;

					payload["variables"]["cursor"] = json!(cursor);
					next_chunk = self.get_chunk_with_payload(&payload).await;
				} else {
					break;
				}
			};
		})
	}

	pub async fn get_chunk_with_payload(
		&self,
		payload: &serde_json::Value,
	) -> Option<GqlVideoContentResponse> {
		let client = reqwest::Client::new();
		let comments = client
			.post("https://gql.twitch.tv/gql")
			.header("Client-ID", std::env::var("CLIENT_ID").unwrap())
			.json(&payload)
			.send()
			.await
			.ok()?;

		let comments: GqlResponse<GqlVideoContentResponse> = comments.json().await.ok()?;

		Some(comments.data)
	}

	pub async fn get_next_chunk(&self) -> Option<GqlVideoContentResponse> {
		let client = reqwest::Client::new();
		let comments = client
			.post("https://gql.twitch.tv/gql")
			.header("Client-ID", std::env::var("CLIENT_ID").unwrap())
			.json(&json!(
				{
					"operationName": "VideoCommentsByOffsetOrCursor",
					"variables": {
						"videoID": self.video_id.to_string(),
						"contentOffsetSeconds": 0,
					},
					"extensions": {
						"persistedQuery": {
							"version": 1,
							"sha256Hash":
								"b70a3591ff0f4e0313d126c6a1502d79a1c02baebb288227c582044aa76adf6a",
						},
					},
				}
			))
			.send()
			.await
			.ok()?;

		let content: GqlResponse<GqlVideoContentResponse> = comments.json().await.ok()?;

		Some(content.data)
	}
}

pub struct VideoIterator {
	pub author: String,
	pub author_id: i64,
}

impl VideoIterator {
	pub fn new(author: String, author_id: i64) -> Self {
		Self { author, author_id }
	}

	pub fn batch(&mut self) -> Pin<Box<impl Stream<Item = Vec<Video>> + '_>> {
		Box::pin(stream! {
			let mut next_chunk = self.get_next_chunk().await.unwrap_or(vec![]);

			loop {
				let last = match next_chunk.last() {
					Some(last) => last,
					None => break,
				};


				let cursor = last.cursor.clone();
				let id = last.id.clone();
				let last_video = Video::new(id, self.author.clone(), self.author_id, cursor, last.created_at);

				yield next_chunk;

				next_chunk = last_video.get_next_chunk().await.unwrap_or(vec![]);
			};
		})
	}

	pub async fn get_next_chunk(&self) -> Option<Vec<Video>> {
		let client = reqwest::Client::new();
		let videos = client
			.post("https://gql.twitch.tv/gql")
			.header("Client-ID", std::env::var("CLIENT_ID").unwrap())
			.json(&json!({
				"operationName": "FilterableVideoTower_Videos",
				"variables": {
					"limit": 30,
					"channelOwnerLogin": self.author,
					"broadcastType": "ARCHIVE",
					"videoSort": "TIME",
				},
				"extensions": {
					"persistedQuery": {
						"version": 1,
						"sha256Hash":
							"a937f1d22e269e39a03b509f65a7490f9fc247d7f83d6ac1421523e3b68042cb",
					},
				},
			}))
			.send()
			.await
			.ok()?;

		let videos: GqlResponse<GqlTrackedUserResponse> = videos.json().await.ok()?;

		videos.data.user.videos.and_then(|videos| {
			let videos = videos
				.edges
				.into_iter()
				.map(|edge| {
					Video::new(
						edge.node.id,
						self.author.clone(),
						self.author_id,
						edge.cursor,
						edge.node.created_at,
					)
				})
				.collect::<Vec<Video>>();

			if videos.len() > 0 {
				Some(videos)
			} else {
				None
			}
		})
	}
}
