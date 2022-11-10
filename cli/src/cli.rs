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
	/// The channel to download. Specify multiple times to download multiple channels.
	#[clap(short = 'c', long, required = true)]
	pub channel: Vec<String>,

	/// The number of threads to use
	#[clap(short = 't', long, default_value_t = 10)]
	pub threads: usize,

	/// Whether to print download progress
	/// Always true if --output or --stdout is specified
	#[clap(short = 'q', long, default_value_t = false)]
	pub quiet: bool,

	/// Whether to pipe data to stdout
	/// Overridden by --output and --postgres
	#[clap(short = 's', long)]
	pub stdout: bool,

	/// Downloads the first n videos from each channel
	#[clap(short = 'l', long)]
	pub limit: Option<usize>,

	/// The file to pipe data to
	/// If not specified, data will be printed to stdout
	/// Overridden by --postgres
	#[clap(alias = "out", short = 'o', long, value_hint = ValueHint::FilePath)]
	pub output: Option<PathBuf>,

	/// The PostgreSQL connection string (leave blank to use DATABASE_URL env)
	/// This will take precedence over all other output arguments
	#[clap(alias = "pg", short = 'p', long)]
	pub postgres: Option<Option<String>>,

	/// The Twitch client ID to use in the request headers
	/// If not specified, CLIENT_ID env will be used, otherwise a default
	#[clap(alias = "id", short = 'i', long)]
	pub client_id: Option<String>,
}
