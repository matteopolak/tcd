use async_trait::async_trait;
use prisma_client_rust::QueryError;
use serde::Deserialize;
use serde_json::json;

use crate::prisma::PrismaClient;
use crate::{
	gql::{GqlChannel, GqlResponse, GqlUser},
	video::VideoIterator,
};

pub struct Channel {
	pub id: i64,
	pub username: String,
}

#[derive(Deserialize)]
pub struct GqlChannelResponse {
	pub user: GqlChannel,
}

#[derive(Deserialize)]
pub struct GqlUserResponse {
	#[serde(rename(deserialize = "targetUser"))]
	pub user: GqlUser,
}

#[async_trait]
pub trait ChannelExt {
	fn new(id: i64, username: String) -> Self;
	fn get_videos(&self) -> VideoIterator;
	async fn from_username(username: String) -> Result<Channel, reqwest::Error>;
	async fn save(&self, client: &PrismaClient) -> Result<(), QueryError>;
}

#[async_trait]
impl ChannelExt for Channel {
	fn new(id: i64, username: String) -> Self {
		Self { id, username }
	}

	fn get_videos(&self) -> VideoIterator {
		VideoIterator::new(self.username.clone(), self.id)
	}

	async fn save(&self, client: &PrismaClient) -> Result<(), QueryError> {
		client
			.user()
			.create(self.id, self.username.clone(), vec![])
			.exec()
			.await
			.ok();

		Ok(())
	}

	async fn from_username(username: String) -> Result<Self, reqwest::Error> {
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
