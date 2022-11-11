use std::path::PathBuf;

use clap::{Parser, ValueHint};

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
pub struct Args {
	/// The channel(s) to download
	#[clap(short = 'c', long, required = true)]
	pub channel: Vec<String>,

	/// The number of threads to use
	#[clap(short = 't', long, default_value_t = 10)]
	pub threads: usize,

	/// Whether to print download progress
	#[clap(short = 'q', long, default_value_t = false)]
	pub quiet: bool,

	/// If specified, pipes data to stdout
	#[clap(short = 's', long)]
	pub stdout: bool,

	/// Downloads the first n videos from each channel
	#[clap(short = 'l', long)]
	pub limit: Option<usize>,

	/// If specified, pipes data to the file
	#[clap(alias = "out", short = 'o', long, value_hint = ValueHint::FilePath)]
	pub output: Option<PathBuf>,

	/// The PostgreSQL connection string (leave blank to use DATABASE_URL)
	#[clap(alias = "pg", short = 'p', long)]
	pub postgres: Option<Option<String>>,

	/// The Twitch client ID to use in the request headers
	#[clap(alias = "id", short = 'i', long)]
	pub client_id: Option<String>,
}
