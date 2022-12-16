use std::path::PathBuf;

use clap::{ArgGroup, Parser, ValueEnum, ValueHint};
use serde::Deserialize;

#[derive(Deserialize, Clone)]
#[serde(remote = "tcd::gql::prelude::Format")]
pub enum Format {
	JsonLines,
	Csv,
}

impl ValueEnum for Format {
	fn value_variants<'a>() -> &'a [Format] {
		&[Format::Csv, Format::JsonLines]
	}

	fn from_str(input: &str, ignore_case: bool) -> Result<Self, String> {
		if ignore_case {
			match input.to_lowercase().as_str() {
				"jsonl" => Ok(Format::JsonLines),
				"csv" => Ok(Format::Csv),
				_ => Err(format!("{input} is not a valid format")),
			}
		} else {
			match input {
				"jsonl" => Ok(Format::JsonLines),
				"csv" => Ok(Format::Csv),
				_ => Err(format!("{input} is not a valid format")),
			}
		}
	}

	fn to_possible_value(&self) -> Option<clap::builder::PossibleValue> {
		Some(match self {
			Format::JsonLines => clap::builder::PossibleValue::new("jsonl"),
			Format::Csv => clap::builder::PossibleValue::new("csv"),
		})
	}
}

impl From<Format> for tcd::gql::prelude::Format {
	fn from(format: Format) -> Self {
		match format {
			Format::JsonLines => tcd::gql::prelude::Format::JsonLines,
			Format::Csv => tcd::gql::prelude::Format::Csv,
		}
	}
}

impl From<&Format> for tcd::gql::prelude::Format {
	fn from(format: &Format) -> Self {
		match format {
			Format::JsonLines => tcd::gql::prelude::Format::JsonLines,
			Format::Csv => tcd::gql::prelude::Format::Csv,
		}
	}
}

impl std::fmt::Display for Format {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		match self {
			Format::JsonLines => write!(f, "jsonl"),
			Format::Csv => write!(f, "csv"),
		}
	}
}

#[allow(clippy::option_option)]
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

	/// If specified, polls for new videos every `poll` seconds
	#[clap(short = 'e', long, default_value_t = false)]
	pub live: bool,

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
	#[clap(short = 'v', long)]
	pub video: Vec<i64>,

	/// The number of minutes to wait between polls (`live` only)
	#[clap(short = 'w', long, default_value_t = 30.)]
	pub wait: f64,
}
