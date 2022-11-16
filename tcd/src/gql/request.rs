use crate::gql::prelude::string;
use serde::Serialize;

#[derive(Serialize)]
pub struct GqlVideoMetadataVariables<'a> {
	#[serde(rename(serialize = "channelLogin"))]
	pub username: &'a str,
	#[serde(with = "string", rename(serialize = "videoID"))]
	pub video_id: &'a i64,
}

#[derive(Serialize)]
pub struct GqlViewerCardVariables<'a> {
	#[serde(with = "string", rename(serialize = "channelID"))]
	pub channel_id: i64,
	#[serde(rename(serialize = "channelLogin"))]
	pub channel_name: &'a str,
	#[serde(rename(serialize = "hasChannelID"))]
	pub has_channel_id: bool,
	#[serde(rename(serialize = "giftRecipientLogin"))]
	pub username: &'a str,
	#[serde(rename(serialize = "isViewerBadgeCollectionEnabled"))]
	pub badge_collection: bool,
	#[serde(rename(serialize = "withStandardGifting"))]
	pub standard_gifting: bool,
}

#[allow(clippy::struct_excessive_bools)]
#[derive(Serialize)]
pub struct GqlPlayerContextVariables<'a> {
	pub channel: &'a str,
	#[serde(rename(serialize = "isLive"))]
	pub is_live: bool,
	#[serde(rename(serialize = "hasCollection"))]
	pub has_collection: bool,
	#[serde(rename(serialize = "collectionID"))]
	pub collection_id: &'a str,
	#[serde(rename(serialize = "videoID"))]
	pub video_id: &'a str,
	#[serde(rename(serialize = "hasVideo"))]
	pub has_video: bool,
	pub slug: &'a str,
	#[serde(rename(serialize = "hasClip"))]
	pub has_clip: bool,
}

#[derive(Serialize)]
pub struct GqlVideoFilterVariables<'a> {
	pub limit: usize,
	#[serde(rename(serialize = "channelOwnerLogin"))]
	pub username: &'a str,
	#[serde(rename(serialize = "broadcastType"))]
	pub r#type: &'static str,
	#[serde(rename(serialize = "videoSort"))]
	pub sort: &'static str,
	pub cursor: Option<&'a str>,
}

#[derive(Serialize)]
pub struct GqlVideoCommentsByOffsetVariables {
	#[serde(with = "string", rename(serialize = "videoID"))]
	pub video_id: i64,
	#[serde(rename(serialize = "contentOffsetSeconds"))]
	pub offset: i64,
}

#[derive(Serialize)]
pub struct GqlVideoCommentsByCursorVariables<'a> {
	#[serde(with = "string", rename(serialize = "videoID"))]
	pub video_id: i64,
	pub cursor: &'a str,
}

#[allow(clippy::module_name_repetitions)]
#[derive(Serialize)]
pub struct GqlRequest<V> {
	#[serde(rename(serialize = "operationName"))]
	pub operation_name: &'static str,
	pub variables: V,
	pub extensions: GqlRequestExtensions,
}

#[derive(Serialize)]
pub struct GqlRequestExtensions {
	#[serde(rename(serialize = "persistedQuery"))]
	pub persisted_query: GqlRequestPersistedQuery,
}

#[derive(Serialize)]
pub struct GqlRequestPersistedQuery {
	#[serde(rename(serialize = "version"))]
	pub version: u8,
	#[serde(rename(serialize = "sha256Hash"))]
	pub sha256_hash: &'static str,
}
