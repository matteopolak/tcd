#![feature(future_join)]
mod cli;

use std::{
	fs::File,
	io::{BufWriter, Write},
	sync::Mutex,
};

use clap::Parser;
use cli::Args;
use dotenv::dotenv;
use futures::StreamExt;
use prisma_client_rust::Direction;

use tcd::{
	channel::Channel,
	gql::prelude::{Paginate, Save, WriteChunk},
	prisma,
	video::Video,
};

#[tokio::main]
async fn main() {
	dotenv().unwrap();

	let args = Args::parse();

	let mut headers = reqwest::header::HeaderMap::new();

	headers.insert(
		"Client-ID",
		reqwest::header::HeaderValue::from_str(
			&std::env::var("CLIENT_ID").expect("CLIENT_ID not set"),
		)
		.expect("Invalid CLIENT_ID header value"),
	);

	let http = reqwest::ClientBuilder::new()
		.default_headers(headers)
		.build()
		.expect("Failed to build HTTP client");

	if let Some(postgres) = &args.postgres {
		std::env::set_var("DATABASE_URL", postgres);

		use_pg(http, args).await;
	} else {
		use_writer(http, args).await;
	}
}

async fn use_writer(http: reqwest::Client, mut args: Args) {
	// Suppress logs when writing to a file or stdout
	args.quiet = true;

	let stream: Mutex<BufWriter<Box<dyn Write>>> = if let Some(path) = args.output {
		match File::options().append(true).create(true).open(path) {
			Ok(file) => Mutex::new(BufWriter::new(Box::new(file))),
			Err(e) => {
				panic!("Failed to open output file: {}", e);
			}
		}
	} else {
		Mutex::new(BufWriter::new(Box::new(std::io::stdout())))
	};

	stream
		.lock()
		.unwrap()
		.write("video_id,comment_id,commenter_id,created_at,text".as_bytes())
		.expect("Failed to write to output file");

	for channel_name in args.channel {
		let channel = match Channel::from_username(&channel_name).await {
			Ok(Some(channel)) => channel,
			Ok(None) => {
				eprintln!("Channel {} not found", channel_name);
				continue;
			}
			Err(e) => {
				eprintln!("Failed to fetch channel {}: {}", channel_name, e);
				continue;
			}
		};

		let mut stop = false;
		let mut videos = channel.paginate(&http);
		let mut limit = args.limit.unwrap_or(0);

		while let Some(container) = videos.next().await {
			let container = match container {
				Ok(container) => container,
				Err(e) => {
					eprintln!("Failed to fetch videos: {:?}", e);
					break;
				}
			};

			let mut videos = container.edges;

			// If the remaining videos to download is greater than 0,
			// update the counter and stop if it reaches 0
			if limit > 0 {
				if videos.len() > limit {
					videos.truncate(limit);
					stop = true;
				}

				limit -= videos.len();
			}

			futures::stream::iter(
				videos
					.into_iter()
					.map(|v| Video::from(v))
					.map(|v| v.write_to_stream(&http, &stream)),
			)
			.buffer_unordered(args.threads)
			.collect::<Vec<_>>()
			.await;

			if stop {
				break;
			}
		}
	}
}

async fn use_pg(http: reqwest::Client, args: Args) {
	let client = match prisma::new_client().await {
		Ok(client) => client,
		Err(err) => panic!("Failed to connect to database: {}", err),
	};

	for channel_name in args.channel {
		let channel = match Channel::from_username(&channel_name).await {
			Ok(Some(channel)) => channel,
			Ok(None) => {
				eprintln!("Channel {} not found", channel_name);
				continue;
			}
			Err(err) => {
				eprintln!("Failed to fetch channel {}: {}", channel_name, err);
				continue;
			}
		};

		if let Err(e) = channel.save(&client).await {
			eprintln!("Failed to save channel {}: {}", channel_name, e);
		}

		let start_at = match client
			.video()
			.find_many(vec![prisma::video::WhereParam::AuthorIdEquals(channel.id)])
			.order_by(prisma::video::OrderByParam::CreatedAt(Direction::Asc))
			.take(1)
			.exec()
			.await
			.and_then(|mut v| {
				if v.is_empty() {
					Ok(None)
				} else {
					Ok(Some(v.remove(0)))
				}
			}) {
			Ok(Some(video)) => video.created_at,
			Ok(None) => chrono::DateTime::<chrono::Utc>::MIN_UTC
				.with_timezone(&chrono::FixedOffset::east(0)),
			Err(e) => panic!("Failed to fetch latest video for {}: {}", channel_name, e),
		};

		let mut stop = false;
		let mut videos = channel.paginate(&http);
		let mut limit = args.limit.unwrap_or(0);

		while let Some(container) = videos.next().await {
			let container = match container {
				Ok(container) => container,
				Err(e) => {
					eprintln!("Failed to fetch videos: {:?}", e);
					break;
				}
			};

			let mut videos = container.edges;

			// If they were all created before the newest stored video, stop
			// after re-checking them all (just in case the download was stopped)
			if videos.iter().all(|v| v.node.created_at < start_at) {
				stop = true;
			}

			// If the remaining videos to download is greater than 0,
			// update the counter and stop if it reaches 0
			if limit > 0 {
				if videos.len() > limit {
					videos.truncate(limit);
					stop = true;
				}

				limit -= videos.len();
			}

			futures::stream::iter(videos.into_iter().map(|v| Video::from(v)).map(|v| async {
				v.save(&client).await.ok();
				v.write_to_pg(&http, &client, !args.quiet).await
			}))
			.buffer_unordered(args.threads)
			.collect::<Vec<_>>()
			.await;

			if stop {
				break;
			}
		}
	}
}
