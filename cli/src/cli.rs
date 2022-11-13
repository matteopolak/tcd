use std::path::PathBuf;

use clap::{ArgGroup, Parser, ValueEnum, ValueHint};
use serde::Deserialize;

#[derive(ValueEnum, Deserialize, Clone)]
#[serde(remote = "tcd::gql::prelude::Format")]
pub enum Format {
	Json,
	Csv,
}

impl From<Format> for tcd::gql::prelude::Format {
	fn from(format: Format) -> Self {
		match format {
			Format::Json => tcd::gql::prelude::Format::Json,
			Format::Csv => tcd::gql::prelude::Format::Csv,
		}
	}
}

impl std::fmt::Display for Format {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		match self {
			Format::Json => write!(f, "json"),
			Format::Csv => write!(f, "csv"),
		}
	}
}

#[derive(Parser)]
#[clap(
	author,
	version,
	about,
	help_template = "
{author-with-newline}{about-with-newline}
{usage-heading} {usage}

{all-args}{after-help}
"
)]
#[clap(group(ArgGroup::new("out").required(false).args(&["output", "postgres", "stdout"])))]
#[clap(group(ArgGroup::new("in").required(true).args(&["channel", "video"])))]
pub struct Args {
	/// The channel(s) to download
	#[clap(short = 'c', long)]
	pub channel: Vec<String>,

	/// The Twitch client ID to use in the request headers
	#[clap(alias = "id", short = 'i', long)]
	pub client_id: Option<String>,

	/// Used with --output or --stdout
	#[clap(alias = "fmt", short = 'f', long, default_value_t = Format::Csv)]
	pub format: Format,

	/// Downloads the first n videos from each channel
	#[clap(short = 'l', long)]
	pub limit: Option<usize>,

	/// If specified, pipes data to the file
	#[clap(alias = "out", short = 'o', long, value_hint = ValueHint::FilePath)]
	pub output: Option<PathBuf>,

	/// The PostgreSQL connection string [default: DATABASE_URL env]
	#[clap(alias = "pg", short = 'p', long)]
	pub postgres: Option<Option<String>>,

	/// Whether to print download progress
	#[clap(short = 'q', long, default_value_t = false)]
	pub quiet: bool,

	/// If specified, pipes data to stdout
	#[clap(short = 's', long)]
	pub stdout: bool,

	/// The number of threads to use
	#[clap(short = 't', long, default_value_t = 10)]
	pub threads: usize,

	/// The video ids to download the chat for
	#[clap(alias = "vid", short = 'v', long)]
	pub video: Vec<i64>,
}
