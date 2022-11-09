use std::{collections::HashMap, pin::Pin};

use async_stream::try_stream;
use async_trait::async_trait;
use chrono::{DateTime, FixedOffset};
use futures::{join, Stream, StreamExt};
use prisma_client_rust::QueryError;

use crate::{
	gql::{
		prelude::{Chunk, ChunkError, Paginate, Save, SaveChunk},
		request::{
			GqlRequest, GqlRequestExtensions, GqlRequestPersistedQuery,
			GqlVideoCommentsByCursorVariables, GqlVideoCommentsByOffsetVariables,
		},
		structs::{
			GqlComment, GqlEdge, GqlEdgeContainer, GqlResponse, GqlVideo, GqlVideoContentResponse,
		},
	},
	prisma::{self, PrismaClient},
};

/// A video on Twitch
#[derive(Debug)]
pub struct Video {
	pub id: i64,
	pub author: String,
	pub author_id: i64,
	pub cursor: Option<String>,
	pub created_at: DateTime<FixedOffset>,
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
			author: video.node.user.username,
			author_id: video.node.user.id,
			cursor: video.cursor,
			created_at: video.node.created_at,
		}
	}
}

#[async_trait]
impl Chunk<GqlEdgeContainer<GqlComment>> for Video {
	/// Gets the comments for the video from a cursor
	async fn chunk_by_cursor(
		&self,
		http: &reqwest::Client,
		cursor: &str,
	) -> Result<GqlEdgeContainer<GqlComment>, ChunkError> {
		let response = http
			.post("https://gql.twitch.tv/gql")
			.json(&GqlRequest {
				operation_name: "VideoCommentsByOffsetOrCursor",
				variables: GqlVideoCommentsByCursorVariables {
					video_id: self.id,
					cursor: cursor,
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
			.map_err(|e| ChunkError::Reqwest(e))?;

		let body: GqlResponse<GqlVideoContentResponse> =
			response.json().await.map_err(|e| ChunkError::Reqwest(e))?;

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
			.map_err(|e| ChunkError::Reqwest(e))?;

		let body: GqlResponse<GqlVideoContentResponse> =
			response.json().await.map_err(|e| ChunkError::Reqwest(e))?;

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
	) -> Pin<Box<dyn Stream<Item = Result<GqlEdgeContainer<GqlComment>, ChunkError>> + '_>> {
		Box::pin(try_stream! {
			let mut cursor: Option<String> = None;

			loop {
				let data = match cursor {
					Some(ref cursor) => self.chunk_by_cursor(http, cursor).await?,
					None => self.first_chunk(http).await?,
				};

				cursor = match data.edges.last() {
					Some(edge) => edge.cursor.clone(),
					None => None,
				};

				yield data;

				if cursor.is_none() {
					break;
				}
			}
		})
	}
}

#[async_trait(?Send)]
impl SaveChunk<GqlComment> for Video {
	/// Saves the comments for a video to the database
	async fn save_chunks(
		self,
		client: &PrismaClient,
		http: &reqwest::Client,
		verbose: bool,
	) -> Result<(), ChunkError> {
		let mut stream = self.paginate(http);
		let video_id = self.id;

		while let Some(Ok(chunk)) = stream.next().await {
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

			for comment in chunk.edges {
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

			let (users, comments, fragments) = join!(
				client
					.user()
					.create_many(users.into_values().into_iter().collect())
					.skip_duplicates()
					.exec(),
				client
					.comment()
					.create_many(comments)
					.skip_duplicates()
					.exec(),
				client
					.comment_fragment()
					.create_many(fragments)
					.skip_duplicates()
					.exec()
			);

			if verbose {
				println!(
					"Saved {} users, {} comments, and {} fragments",
					users.map_err(|e| ChunkError::Prisma(e))?,
					comments.map_err(|e| ChunkError::Prisma(e))?,
					fragments.map_err(|e| ChunkError::Prisma(e))?
				);
			}
		}

		Ok(())
	}
}
