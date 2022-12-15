#![feature(closure_track_caller)]
#![warn(clippy::pedantic)]

mod cli;
mod pg;
mod writer;

use clap::Parser;
use cli::Args;
use dotenv::dotenv;

static CLIENT_ID: &str = "kimne78kx3ncx6brgo4mv6wki5h1ko";

#[tokio::main]
async fn main() {
	dotenv().ok();

	let args = Args::parse();

	let mut headers = reqwest::header::HeaderMap::new();
	let client_id = std::env::var("CLIENT_ID");

	headers.insert(
		"Client-ID",
		// First, try to get the client ID from the command-line arguments
		reqwest::header::HeaderValue::from_str(if let Some(client_id) = args.client_id.as_ref() {
			client_id
		// Otherwise, check the environment variable CLIENT_ID
		} else if let Ok(client_id) = client_id.as_ref() {
			client_id
		// Otherwise, use the default client ID
		} else {
			CLIENT_ID
		})
		.expect("Invalid CLIENT_ID header value"),
	);

	let http = reqwest::ClientBuilder::new()
		.default_headers(headers)
		.build()
		.expect("Failed to build HTTP client");

	if let Some(postgres) = &args.postgres {
		if let Some(postgres) = postgres {
			std::env::set_var("DATABASE_URL", postgres);
		} else {
			std::env::var("DATABASE_URL").expect("DATABASE_URL env not set, either set it or specify a connection string to --postgres");
		}

		crate::pg::run(http, args).await;
	} else {
		crate::writer::run(http, args).await;
	}
}
