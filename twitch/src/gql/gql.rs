use chrono::{DateTime, FixedOffset, Utc};
use serde::Deserialize;

// https://github.com/serde-rs/json/issues/329
mod string {
	use std::fmt::Display;
	use std::str::FromStr;

	use serde::{de, Deserialize, Deserializer};

	pub fn deserialize<'de, T, D>(deserializer: D) -> Result<T, D::Error>
	where
		T: FromStr,
		T::Err: Display,
		D: Deserializer<'de>,
	{
		String::deserialize(deserializer)?
			.parse()
			.map_err(de::Error::custom)
	}
}

#[derive(Deserialize, Debug)]
pub struct GqlResponse<T> {
	pub data: T,
	pub extensions: GqlExtensions,
}

#[derive(Deserialize, Debug)]
pub struct GqlExtensions {
	#[serde(rename(deserialize = "durationMilliseconds"))]
	pub duration_ms: u32,
	#[serde(rename(deserialize = "operationName"))]
	pub operation_name: String,
	#[serde(rename(deserialize = "requestID"))]
	pub request_id: String,
}

#[derive(Deserialize, Debug)]
pub struct GqlChannel {
	#[serde(with = "string")]
	pub id: i64,
	#[serde(rename(deserialize = "login"))]
	pub username: String,
}

#[derive(Deserialize, Debug)]
pub struct GqlCommenter {
	#[serde(with = "string")]
	pub id: i64,
	#[serde(rename(deserialize = "login"))]
	pub username: String,
}

#[derive(Deserialize, Debug)]
pub struct GqlUser {
	#[serde(with = "string")]
	pub id: i64,
	#[serde(rename(deserialize = "login"))]
	pub username: String,
	#[serde(rename(deserialize = "displayName"))]
	pub display_name: String,
	#[serde(rename(deserialize = "createdAt"))]
	pub created_at: DateTime<Utc>,
}

#[derive(Deserialize, Debug)]
pub struct GqlPageInfo {
	#[serde(rename(deserialize = "hasNextPage"))]
	pub has_next_page: bool,
}

#[derive(Deserialize, Debug)]
pub struct GqlEdge<T> {
	pub cursor: Option<String>,
	pub node: T,
}

#[derive(Deserialize, Debug)]
pub struct GqlEdgeContainer<T> {
	pub edges: Vec<GqlEdge<T>>,
	#[serde(rename(deserialize = "pageInfo"))]
	pub page_info: GqlPageInfo,
}

#[derive(Deserialize, Debug)]
pub struct GqlVideo {
	#[serde(with = "string")]
	pub id: i64,
	#[serde(rename(deserialize = "lengthSeconds"))]
	pub length: u32,
	#[serde(rename(deserialize = "publishedAt"))]
	pub created_at: DateTime<FixedOffset>,
	#[serde(rename(deserialize = "owner"))]
	pub user: GqlChannel,
}

#[derive(Deserialize, Debug)]
pub struct GqlEmote {
	#[serde(rename(deserialize = "emoteID"))]
	pub emote_id: String,
}

#[derive(Deserialize, Debug)]
pub struct GqlCommentMessageFragment {
	pub emote: Option<GqlEmote>,
	pub text: String,
}

#[derive(Deserialize, Debug)]
pub struct GqlCommentMessage {
	pub fragments: Vec<GqlCommentMessageFragment>,
}

#[derive(Deserialize, Debug)]
pub struct GqlComment {
	pub id: String,
	pub commenter: Option<GqlCommenter>,
	#[serde(rename(deserialize = "contentOffsetSeconds"))]
	pub offset: u32,
	#[serde(rename(deserialize = "createdAt"))]
	pub created_at: DateTime<FixedOffset>,
	pub message: GqlCommentMessage,
}

#[derive(Deserialize, Debug)]
pub struct GqlVideoContent {
	#[serde(with = "string")]
	pub id: i64,
	pub comments: GqlEdgeContainer<GqlComment>,
}

#[derive(Deserialize, Debug)]
pub struct GqlTrackedUser {
	#[serde(with = "string")]
	pub id: i64,
	pub videos: Option<GqlEdgeContainer<GqlVideo>>,
}
