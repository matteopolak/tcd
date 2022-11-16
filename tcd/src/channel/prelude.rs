use std::pin::Pin;

use async_stream::try_stream;
use async_trait::async_trait;
use futures::Stream;
use prisma_client_rust::QueryError;

use crate::{
	gql::{
		prelude::{Chunk, ChunkError, PaginateMut, Save},
		request::{
			GqlPlayerContextVariables, GqlRequest, GqlRequestExtensions, GqlRequestPersistedQuery,
			GqlVideoFilterVariables, GqlViewerCardVariables,
		},
		structs::{
			GqlChannelResponse, GqlEdgeContainer, GqlResponse, GqlTrackedUserResponse,
			GqlUserResponse, GqlVideo,
		},
	},
	prisma::PrismaClient,
};

#[derive(Debug, PartialEq)]
pub struct Channel {
	pub id: i64,
	pub username: String,
	pub last_video_id: Option<i64>,
}

impl Channel {
	/// Gets a channel from a username
	#[allow(clippy::missing_errors_doc)]
	pub async fn from_username(
		http: &reqwest::Client,
		username: &str,
	) -> Result<Option<Self>, reqwest::Error> {
		let user = http
			.post("https://gql.twitch.tv/gql")
			.json(&GqlRequest {
				operation_name: "PlayerTrackingContextQuery",
				variables: GqlPlayerContextVariables {
					channel: username,
					is_live: true,
					has_collection: false,
					collection_id: "",
					video_id: "",
					has_video: false,
					slug: "",
					has_clip: false,
				},
				extensions: GqlRequestExtensions {
					persisted_query: GqlRequestPersistedQuery {
						version: 1,
						sha256_hash:
							"3fbf508886ff5e008cb94047acc752aad7428c07b6055995604de16c4b01160a",
					},
				},
			})
			.send()
			.await?;

		let user: GqlResponse<GqlChannelResponse> = user.json().await?;
		let user = match user.data.user {
			Some(user) => user,
			None => return Ok(None),
		};

		let user = http
			.post("https://gql.twitch.tv/gql")
			.json(&GqlRequest {
				operation_name: "ViewerCard",
				variables: GqlViewerCardVariables {
					channel_id: user.id,
					channel_name: &user.username,
					has_channel_id: true,
					username: &user.username,
					badge_collection: true,
					standard_gifting: false,
				},
				extensions: GqlRequestExtensions {
					persisted_query: GqlRequestPersistedQuery {
						version: 1,
						sha256_hash:
							"20e51233313878f971daa32dfc039b2e2183822e62c13f47c48448d5d5e4f5e9",
					},
				},
			})
			.send()
			.await?;

		let user: GqlResponse<GqlUserResponse> = user.json().await?;

		Ok(Some(Self {
			id: user.data.user.id,
			username: user.data.user.username,
			last_video_id: None,
		}))
	}
}

#[async_trait]
impl Save for Channel {
	async fn save(&self, client: &PrismaClient) -> Result<(), QueryError> {
		client
			.user()
			.create(self.id, self.username.clone(), vec![])
			.exec()
			.await?;

		Ok(())
	}
}

#[async_trait]
impl Chunk<GqlEdgeContainer<GqlVideo>> for Channel {
	/// Gets the next chunk of videos for the channel from a cursor
	async fn chunk_by_cursor(
		&self,
		http: &reqwest::Client,
		cursor: &str,
	) -> Result<GqlEdgeContainer<GqlVideo>, ChunkError> {
		let response = http
			.post("https://gql.twitch.tv/gql")
			.json(&GqlRequest {
				operation_name: "FilterableVideoTower_Videos",
				variables: GqlVideoFilterVariables {
					limit: 30,
					username: &self.username,
					r#type: "ARCHIVE",
					sort: "TIME",
					cursor: Some(cursor),
				},
				extensions: GqlRequestExtensions {
					persisted_query: GqlRequestPersistedQuery {
						version: 1,
						sha256_hash:
							"a937f1d22e269e39a03b509f65a7490f9fc247d7f83d6ac1421523e3b68042cb",
					},
				},
			})
			.send()
			.await
			.map_err(ChunkError::Reqwest)?;

		let body: GqlResponse<GqlTrackedUserResponse> =
			response.json().await.map_err(ChunkError::Reqwest)?;

		body.data.user.videos.ok_or(ChunkError::DataMissing)
	}

	/// Gets the first chunk of videos for the channel
	async fn first_chunk(
		&self,
		http: &reqwest::Client,
	) -> Result<GqlEdgeContainer<GqlVideo>, ChunkError> {
		let response = http
			.post("https://gql.twitch.tv/gql")
			.json(&GqlRequest {
				operation_name: "FilterableVideoTower_Videos",
				variables: GqlVideoFilterVariables {
					limit: 30,
					username: &self.username,
					r#type: "ARCHIVE",
					sort: "TIME",
					cursor: None,
				},
				extensions: GqlRequestExtensions {
					persisted_query: GqlRequestPersistedQuery {
						version: 1,
						sha256_hash:
							"a937f1d22e269e39a03b509f65a7490f9fc247d7f83d6ac1421523e3b68042cb",
					},
				},
			})
			.send()
			.await
			.map_err(ChunkError::Reqwest)?;

		let body: GqlResponse<GqlTrackedUserResponse> =
			response.json().await.map_err(ChunkError::Reqwest)?;

		body.data.user.videos.ok_or(ChunkError::DataMissing)
	}
}

impl PaginateMut<GqlVideo> for Channel {
	/// Gets a stream of all videos for the channel
	fn paginate_mut<'a>(
		&'a mut self,
		http: &'a reqwest::Client,
	) -> Pin<Box<dyn Stream<Item = Result<GqlEdgeContainer<GqlVideo>, ChunkError>> + 'a>> {
		Box::pin(try_stream! {
			let mut cursor: Option<String> = None;

			loop {
				let data = match cursor {
					Some(ref cursor) => self.chunk_by_cursor(http, cursor).await?,
					None => self.first_chunk(http).await?,
				};

				cursor = match data.edges.last() {
					Some(edge) => {
						self.last_video_id = Some(edge.node.id.clone());

						edge.cursor.clone()
					},
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
