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
		prelude::{Chunk, ChunkError, Format, Paginate, Save, WriteChunk},
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
#[derive(Debug, PartialEq)]
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
				let users = users.map_err(|e| ChunkError::Prisma(e))?;
				let comments = comments.map_err(|e| ChunkError::Prisma(e))?;
				let fragments = fragments.map_err(|e| ChunkError::Prisma(e))?;

				if users != 0 || comments != 0 || fragments != 0 {
					println!(
						"Saved {} users, {} comments, and {} fragments",
						users, comments, fragments
					);
				}
			}
		}

		Ok(())
	}

	async fn write_to_stream(
		self,
		http: &reqwest::Client,
		stream: &Mutex<BufWriter<impl Write>>,
		format: &Format,
	) -> Result<(), ChunkError> {
		let mut chunks = self
			.paginate(http)
			.map(|c| {
				c.and_then(|c| {
					Ok(c.edges
						.into_iter()
						.filter_map(|c| {
							if let Some(commenter) = c.node.commenter {
								Some(format_data(
									format,
									self.author_id,
									self.id,
									c.node.id,
									commenter.id,
									c.node.created_at,
									c.node
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
						.intersperse(",".to_string())
						.collect::<String>())
				})
			})
			.chunks(5);

		while let Some(chunk) = chunks.next().await {
			let chunk = chunk.into_iter().collect::<Result<Vec<_>, _>>()?;

			let mut stream = stream.lock().unwrap();

			stream.write(b",").map_err(|_| ChunkError::Io)?;

			stream
				.write(chunk.join(",").as_bytes())
				.map_err(|_| ChunkError::Io)?;
		}

		Ok(())
	}
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CommentEntry {
	pub channel_id: i64,
	pub video_id: i64,
	pub comment_id: String,
	pub commenter_id: i64,
	pub created_at: DateTime<FixedOffset>,
	pub text: String,
}

fn format_data(
	format: &Format,
	author_id: i64,
	video_id: i64,
	comment_id: String,
	commenter_id: i64,
	created_at: DateTime<FixedOffset>,
	text: String,
) -> String {
	match format {
		Format::Json => serde_json::to_string(&CommentEntry {
			channel_id: author_id,
			video_id,
			comment_id,
			commenter_id,
			created_at,
			text,
		})
		.unwrap(),
		Format::Csv => {
			format!(
				"{},{},{},{},\"{}\",{:?}",
				author_id, video_id, comment_id, commenter_id, created_at, text
			)
		}
	}
}
