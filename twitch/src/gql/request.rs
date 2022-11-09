use crate::gql::prelude::string;
use serde::Serialize;

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
