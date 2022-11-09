use clap::Parser;

/// Twitch Chat Downloader
#[derive(Parser)]
#[clap(author = "Matthew Polak")]
pub struct Args {
	/// The channel to download. Specify multiple times to download multiple channels.
	#[clap(short = 'c', long, required = true)]
	pub channel: Vec<String>,

	/// The number of threads to use
	#[clap(short = 't', long, default_value_t = 10)]
	pub threads: usize,

	/// Whether to print download progress
	#[clap(short = 'v', long, default_value_t = true)]
	pub verbose: bool,
}
