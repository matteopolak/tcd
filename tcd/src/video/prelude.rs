use std::{
	collections::HashMap,
	io::{BufWriter, Write},
	pin::Pin,
	sync::Mutex,
};

use async_stream::try_stream;
use async_trait::async_trait;
use chrono::{DateTime, FixedOffset};
use futures::{join, Stream, StreamExt};
use prisma_client_rust::QueryError;
use serde::Serialize;

use crate::{
	gql::{
		prelude::{Chunk, ChunkError, Format, Paginate, PaginateFilter, Save, WriteChunk},
		request::{
			GqlRequest, GqlRequestExtensions, GqlRequestPersistedQuery,
			GqlVideoCommentsByCursorVariables, GqlVideoCommentsByOffsetVariables,
			GqlVideoMetadataVariables,
		},
		structs::{
			GqlComment, GqlEdge, GqlEdgeContainer, GqlResponse, GqlVideo, GqlVideoContentResponse,
			GqlVideoMetadataResponse,
		},
	},
	prisma::{self, PrismaClient},
};

/// A video on Twitch
#[derive(Clone, Debug, PartialEq)]
pub struct Video {
	pub id: i64,
	pub title: String,
	pub author: String,
	pub author_id: i64,
	pub cursor: Option<String>,
	pub created_at: DateTime<FixedOffset>,
	pub thumbnail_url: String,
	pub thumbnail: Option<Vec<u8>>,
}

impl Video {
	#[allow(clippy::missing_errors_doc)]
	pub async fn get_thumbnail<'a>(
		&'a mut self,
		http: &reqwest::Client,
	) -> Result<Option<&'a Vec<u8>>, reqwest::Error> {
		if self.thumbnail.is_some() {
			return Ok(self.thumbnail.as_ref());
		}

		let response = http.get(&self.thumbnail_url).send().await?;

		let thumbnail = response.bytes().await?.into_iter().collect();
		self.thumbnail = Some(thumbnail);

		Ok(self.thumbnail.as_ref())
	}
}

#[async_trait]
impl Save for Video {
	/// Saves the video to the database
	async fn save(&self, client: &PrismaClient) -> Result<(), QueryError> {
		client
			.video()
			.create(
				self.id,
				prisma::user::UniqueWhereParam::IdEquals(self.author_id),
				self.created_at,
				vec![],
			)
			.exec()
			.await?;

		Ok(())
	}
}

impl From<GqlEdge<GqlVideo>> for Video {
	/// Converts a GraphQL video edge to a video
	fn from(video: GqlEdge<GqlVideo>) -> Self {
		Self {
			id: video.node.id,
			title: video.node.title,
			author: video.node.user.username,
			author_id: video.node.user.id,
			cursor: video.cursor,
			created_at: video.node.created_at,
			thumbnail_url: video.node.thumbnail_url,
			thumbnail: None,
		}
	}
}

impl From<GqlVideo> for Video {
	/// Converts a GraphQL video to a video
	fn from(video: GqlVideo) -> Self {
		Self {
			id: video.id,
			title: video.title,
			author: video.user.username,
			author_id: video.user.id,
			cursor: None,
			created_at: video.created_at,
			thumbnail_url: video.thumbnail_url,
			thumbnail: None,
		}
	}
}

#[async_trait]
impl Chunk<GqlEdgeContainer<GqlComment>> for Video {
	/// Gets the comments for the video from a cursor
	async fn chunk_by_cursor<'a, S: Into<&'a str> + Send>(
		&self,
		http: &reqwest::Client,
		cursor: S,
	) -> Result<GqlEdgeContainer<GqlComment>, ChunkError> {
		let response = http
			.post("https://gql.twitch.tv/gql")
			.json(&GqlRequest {
				operation_name: "VideoCommentsByOffsetOrCursor",
				variables: GqlVideoCommentsByCursorVariables {
					video_id: self.id,
					cursor: cursor.into(),
				},
				extensions: GqlRequestExtensions {
					persisted_query: GqlRequestPersistedQuery {
						version: 1,
						sha256_hash:
							"b70a3591ff0f4e0313d126c6a1502d79a1c02baebb288227c582044aa76adf6a",
					},
				},
			})
			.send()
			.await
			.map_err(|_| ChunkError::Reqwest)?;

		let body: GqlResponse<GqlVideoContentResponse> =
			response.json().await.map_err(|_| ChunkError::Reqwest)?;

		if let Some(video) = body.data.video {
			Ok(video.comments)
		} else {
			Err(ChunkError::DataMissing)
		}
	}

	/// Gets the first comments for the video
	async fn first_chunk(
		&self,
		http: &reqwest::Client,
	) -> Result<GqlEdgeContainer<GqlComment>, ChunkError> {
		let response = http
			.post("https://gql.twitch.tv/gql")
			.json(&GqlRequest {
				operation_name: "VideoCommentsByOffsetOrCursor",
				variables: GqlVideoCommentsByOffsetVariables {
					video_id: self.id,
					offset: 0,
				},
				extensions: GqlRequestExtensions {
					persisted_query: GqlRequestPersistedQuery {
						version: 1,
						sha256_hash:
							"b70a3591ff0f4e0313d126c6a1502d79a1c02baebb288227c582044aa76adf6a",
					},
				},
			})
			.send()
			.await
			.map_err(|_| ChunkError::Reqwest)?;

		let body: GqlResponse<GqlVideoContentResponse> =
			response.json().await.map_err(|_| ChunkError::Reqwest)?;

		if let Some(video) = body.data.video {
			Ok(video.comments)
		} else {
			Err(ChunkError::DataMissing)
		}
	}
}

impl Paginate<GqlComment> for Video {
	/// Iterates the comments for a video
	fn paginate<'a>(
		&'a self,
		http: &'a reqwest::Client,
	) -> Pin<Box<dyn Stream<Item = Result<GqlEdgeContainer<GqlComment>, ChunkError>> + 'a + Send>> {
		Box::pin(try_stream! {
			let mut cursor: Option<String> = None;

			loop {
				let data = match cursor {
					Some(cursor) => self.chunk_by_cursor(http, cursor.as_str()).await?,
					None => self.first_chunk(http).await?,
				};

				let has_next = data.page_info.has_next_page;

				cursor = match data.edges.last() {
					Some(edge) => edge.cursor.clone(),
					None => None,
				};

				yield data;

				if !has_next {
					break;
				}
			}
		})
	}
}

#[async_trait]
impl WriteChunk<GqlComment> for Video {
	/// Saves the comments for a video to the database
	async fn write_to_pg(
		self,
		http: &reqwest::Client,
		client: &PrismaClient,
		verbose: bool,
	) -> Result<(), ChunkError> {
		let mut comment_chunks = self.paginate(http);
		let video_id = self.id;

		while let Some(Ok(chunk)) = comment_chunks.next().await {
			let mut comments: Vec<(
				String,
				i64,
				i64,
				String,
				::prisma_client_rust::chrono::DateTime<::prisma_client_rust::chrono::FixedOffset>,
				Vec<prisma::comment::SetParam>,
			)> = vec![];
			let mut users: HashMap<i64, (i64, String, Vec<prisma::user::SetParam>)> =
				HashMap::new();

			for comment in chunk.edges {
				let commenter = match comment.node.commenter {
					Some(commenter) => commenter,
					None => continue,
				};

				users.entry(commenter.id).or_insert_with(|| {
					prisma::user::create_unchecked(commenter.id, commenter.username, vec![])
				});

				comments.push(prisma::comment::create_unchecked(
					comment.node.id,
					commenter.id,
					video_id,
					comment
						.node
						.message
						.fragments
						.into_iter()
						.map(|f| f.text)
						.collect::<String>(),
					comment.node.created_at,
					vec![],
				));
			}

			let (users, comments) = join!(
				client
					.user()
					.create_many(users.into_values().into_iter().collect())
					.skip_duplicates()
					.exec(),
				client
					.comment()
					.create_many(comments)
					.skip_duplicates()
					.exec()
			);

			if verbose {
				let users = users.unwrap_or(0);
				let comments = comments.unwrap_or(0);

				if users != 0 || comments != 0 {
					println!("Saved {users} users, {comments} comments");
				}
			}
		}

		Ok(())
	}

	async fn write_to_stream(
		self,
		http: &reqwest::Client,
		stream: &Mutex<BufWriter<impl Write + Send>>,
		format: &Format,
	) -> Result<(), ChunkError> {
		let join_str = "\n";

		let mut chunks = self
			.paginate(http)
			.map(|c| {
				c.map(|c| {
					c.edges
						.into_iter()
						.filter_map(|c| {
							if let Some(commenter) = c.node.commenter {
								Some(format_data(
									format,
									&self.author,
									self.id,
									&c.node.id,
									&commenter.username,
									c.node.created_at,
									&c.node
										.message
										.fragments
										.into_iter()
										.map(|f| f.text)
										.collect::<String>(),
								))
							} else {
								None
							}
						})
						.intersperse(join_str.to_string())
						.collect::<String>()
				})
			})
			.chunks(5);

		while let Some(chunk) = chunks.next().await {
			let chunk = chunk.into_iter().collect::<Result<Vec<_>, _>>()?;
			let mut stream = stream.lock().unwrap();

			stream
				.write(chunk.join(join_str).as_bytes())
				.map_err(|_| ChunkError::Io)?;

			stream
				.write(join_str.as_bytes())
				.map_err(|_| ChunkError::Io)?;
		}

		Ok(())
	}
}

#[derive(Serialize)]
pub struct CommentEntry<'a> {
	pub channel: &'a str,
	pub video_id: i64,
	pub comment_id: &'a str,
	pub commenter: &'a str,
	pub created_at: DateTime<FixedOffset>,
	pub text: &'a str,
}

fn format_data(
	format: &Format,
	author: &String,
	video_id: i64,
	comment_id: &String,
	commenter: &String,
	created_at: DateTime<FixedOffset>,
	text: &String,
) -> String {
	match format {
		Format::JsonLines => serde_json::to_string(&CommentEntry {
			channel: author,
			video_id,
			comment_id,
			commenter,
			created_at,
			text,
		})
		.unwrap(),
		Format::Csv => {
			format!(
				"{},{},{},{},\"{}\",{:?}",
				author, video_id, comment_id, commenter, created_at, text
			)
		}
	}
}

impl PaginateFilter<GqlVideo> for Video {
	// Gets all videos for the channel that are in the given ids
	fn paginate_filter<'a>(
		http: &'a reqwest::Client,
		ids: &'a [i64],
	) -> Pin<Box<dyn Stream<Item = Result<GqlVideo, ChunkError>> + 'a>> {
		Box::pin(try_stream! {
			for id in ids {
				let response = http
					.post("https://gql.twitch.tv/gql")
					.json(&GqlRequest {
						operation_name: "VideoMetadata",
						variables: GqlVideoMetadataVariables {
							username: "",
							video_id: id,
						},
						extensions: GqlRequestExtensions {
							persisted_query: GqlRequestPersistedQuery {
								version: 1,
								sha256_hash:
									"49b5b8f268cdeb259d75b58dcb0c1a748e3b575003448a2333dc5cdafd49adad",
							},
						},
					})
					.send()
					.await
					.map_err(|_| ChunkError::Reqwest)?;

				let body: GqlResponse<GqlVideoMetadataResponse> = response.json().await.map_err(|_| ChunkError::Reqwest)?;

				match body.data.video {
					Some(video) => yield video,
					None => continue,
				}
			}
		})
	}
}
