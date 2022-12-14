use std::{
	io::{BufWriter, Write},
	sync::Mutex,
};

use async_trait::async_trait;
use futures::stream::BoxStream;
use prisma_client_rust::QueryError;

use crate::prisma::PrismaClient;

use super::structs::GqlEdgeContainer;

// https://github.com/serde-rs/json/issues/329
#[allow(clippy::missing_errors_doc)]
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
	) -> BoxStream<'a, Result<GqlEdgeContainer<T>, ChunkError>>;
}

pub trait PaginateMut<T>: Chunk<GqlEdgeContainer<T>> {
	fn paginate_mut<'a>(
		&'a mut self,
		http: &'a reqwest::Client,
	) -> BoxStream<'a, GqlEdgeContainer<T>>;
}

pub trait PaginateFilter<T> {
	fn paginate_filter<'a>(
		http: &'a reqwest::Client,
		ids: &'a [i64],
	) -> BoxStream<'a, Result<T, ChunkError>>;
}

#[derive(Debug, PartialEq)]
pub enum Format {
	JsonLines,
	Csv,
}

#[async_trait]
pub trait WriteChunk<T>: Paginate<T> {
	async fn write_to_pg(
		self,
		http: &reqwest::Client,
		client: &PrismaClient,
		verbose: bool,
	) -> Result<(), ChunkError>;
	async fn write_to_stream(
		self,
		http: &reqwest::Client,
		stream: &Mutex<BufWriter<impl Write + Send>>,
		format: &Format,
	) -> Result<(), ChunkError>;
}

#[async_trait]
pub trait Chunk<T> {
	async fn chunk_by_cursor<'a, S: Into<&'a str> + Send>(
		&self,
		http: &reqwest::Client,
		cursor: S,
	) -> Result<T, ChunkError>;
	async fn first_chunk(&self, http: &reqwest::Client) -> Result<T, ChunkError>;
}

#[derive(Clone, Debug)]
pub enum ChunkError {
	Reqwest,
	Serde,
	Prisma,
	Io,
	Csv,
	DataMissing,
}
