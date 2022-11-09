use clap::Parser;

#[derive(Parser)]
#[clap(author = clap::crate_authors!(), version = clap::crate_version!(), about = clap::crate_description!())]
pub struct Args {
	/// The channel to download. Specify multiple times to download multiple channels.
	#[clap(short = 'c', long, required = true)]
	pub channel: Vec<String>,

	/// The number of threads to use
	#[clap(short = 't', long, default_value_t = 10)]
	pub threads: usize,

	/// Whether to print download progress
	#[clap(short = 'q', long, default_value_t = false)]
	pub quiet: bool,
}
