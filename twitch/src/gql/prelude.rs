use std::pin::Pin;

use async_trait::async_trait;
use futures::Stream;
use prisma_client_rust::QueryError;

use crate::prisma::PrismaClient;

use super::structs::GqlEdgeContainer;

// https://github.com/serde-rs/json/issues/329
pub mod string {
	use std::fmt::Display;
	use std::str::FromStr;

	use serde::{de, Deserialize, Deserializer, Serializer};

	pub fn serialize<T, S>(value: &T, serializer: S) -> Result<S::Ok, S::Error>
	where
		T: Display,
		S: Serializer,
	{
		serializer.collect_str(value)
	}

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

#[async_trait]
pub trait Save {
	async fn save(&self, client: &PrismaClient) -> Result<(), QueryError>;
}

pub trait Paginate<T>: Chunk<GqlEdgeContainer<T>> {
	fn paginate<'a>(
		&'a self,
		http: &'a reqwest::Client,
	) -> Pin<Box<dyn Stream<Item = Result<GqlEdgeContainer<T>, ChunkError>> + '_>>;
}

#[async_trait(?Send)]
pub trait SaveChunk<T>: Paginate<T> {
	async fn save_chunks(
		self,
		client: &PrismaClient,
		http: &reqwest::Client,
		verbose: bool,
	) -> Result<(), ChunkError>;
}

#[async_trait]
pub trait Chunk<T> {
	async fn chunk_by_cursor(&self, http: &reqwest::Client, cursor: &str) -> Result<T, ChunkError>;
	async fn first_chunk(&self, http: &reqwest::Client) -> Result<T, ChunkError>;
}

#[derive(Debug)]
pub enum ChunkError {
	Reqwest(reqwest::Error),
	Serde(serde_json::Error),
	Prisma(QueryError),
	DataMissing,
}
