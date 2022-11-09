use std::pin::Pin;

use async_stream::try_stream;
use async_trait::async_trait;
use futures::Stream;
use prisma_client_rust::QueryError;
use serde_json::json;

use crate::{
	gql::{
		prelude::{Chunk, ChunkError, Paginate, Save},
		request::{
			GqlRequest, GqlRequestExtensions, GqlRequestPersistedQuery, GqlVideoFilterVariables,
		},
		structs::{
			GqlChannelResponse, GqlEdgeContainer, GqlResponse, GqlTrackedUserResponse,
			GqlUserResponse, GqlVideo,
		},
	},
	prisma::PrismaClient,
};

pub struct Channel {
	pub id: i64,
	pub username: String,
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
			.map_err(|e| ChunkError::Reqwest(e))?;

		let body: GqlResponse<GqlTrackedUserResponse> =
			response.json().await.map_err(|e| ChunkError::Reqwest(e))?;

		body.data
			.user
			.videos
			.map_or(Err(ChunkError::DataMissing), |v| Ok(v))
	}

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
			.map_err(|e| ChunkError::Reqwest(e))?;

		let body: GqlResponse<GqlTrackedUserResponse> =
			response.json().await.map_err(|e| ChunkError::Reqwest(e))?;

		body.data
			.user
			.videos
			.map_or(Err(ChunkError::DataMissing), |v| Ok(v))
	}
}

impl Paginate<GqlVideo> for Channel {
	fn paginate<'a>(
		&'a self,
		http: &'a reqwest::Client,
	) -> Pin<Box<dyn Stream<Item = Result<GqlEdgeContainer<GqlVideo>, ChunkError>> + '_>> {
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

impl Channel {
	pub async fn from_username(username: &str) -> Result<Self, reqwest::Error> {
		let client = reqwest::Client::new();
		let user = client
			.post("https://gql.twitch.tv/gql")
			.header("Client-ID", std::env::var("CLIENT_ID").unwrap())
			.json(&json!({
				"operationName": "PlayerTrackingContextQuery",
				"variables": {
					"channel": username,
					"isLive": true,
					"hasCollection": false,
					"collectionID": "",
					"videoID": "",
					"hasVideo": false,
					"slug": "",
					"hasClip": false,
				},
				"extensions": {
					"persistedQuery": {
						"version": 1,
						"sha256Hash":
							"3fbf508886ff5e008cb94047acc752aad7428c07b6055995604de16c4b01160a",
					},
				},
			}))
			.send()
			.await?;

		let user: GqlResponse<GqlChannelResponse> = user.json().await?;
		let user = client
			.post("https://gql.twitch.tv/gql")
			.header("Client-ID", std::env::var("CLIENT_ID").unwrap())
			.json(&json!({
				"operationName": "ViewerCard",
				"variables": {
					"channelID": user.data.user.id.to_string(),
					"channelLogin": user.data.user.username,
					"hasChannelID": true,
					"giftRecipientLogin": user.data.user.username,
					"isViewerBadgeCollectionEnabled": true,
					"withStandardGifting": false,
				},
				"extensions": {
					"persistedQuery": {
						"version": 1,
						"sha256Hash":
							"20e51233313878f971daa32dfc039b2e2183822e62c13f47c48448d5d5e4f5e9",
					},
				},
			}))
			.send()
			.await?;

		let user: GqlResponse<GqlUserResponse> = user.json().await?;

		Ok(Self {
			id: user.data.user.id,
			username: user.data.user.username,
		})
	}
}
