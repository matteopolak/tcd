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
	gql::prelude::{Format, PaginateFilter, PaginateMut, Save, WriteChunk},
	prisma,
	video::Video,
};

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

		use_pg(http, args).await;
	} else {
		use_writer(http, args).await;
	}
}

async fn use_writer_channels(
	http: &reqwest::Client,
	mut channels: Vec<Channel>,
	limit: &mut usize,
	threads: usize,
	stream: Mutex<BufWriter<Box<dyn Write>>>,
	format: &Format,
) -> (Mutex<BufWriter<Box<dyn Write>>>, Vec<Channel>) {
	for channel in channels.iter_mut() {
		let mut stop = false;
		let stop_at = channel.last_video_id.unwrap_or(0);
		let mut videos = channel.paginate_mut(&http);

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
			if *limit > 0 {
				if videos.len() > *limit {
					videos.truncate(*limit);
					stop = true;
				}

				*limit -= videos.len();
			}

			let idx = videos.iter().position(|v| v.node.id == stop_at);

			if let Some(idx) = idx {
				videos.truncate(idx);
				stop = true;
			}

			futures::stream::iter(
				videos
					.into_iter()
					.map(|v| Video::from(v))
					.map(|v| v.write_to_stream(&http, &stream, &format)),
			)
			.buffer_unordered(threads)
			.collect::<Vec<_>>()
			.await;

			if stop {
				break;
			}
		}
	}

	eprintln!("finished scan");

	stream
		.lock()
		.unwrap()
		.flush()
		.expect("Failed to flush output file");

	(stream, channels)
}

async fn use_writer(http: reqwest::Client, mut args: Args) {
	// Suppress logs when writing to a file or stdout
	args.quiet = true;

	let stream: Mutex<BufWriter<Box<dyn Write>>> = if let Some(path) = &args.output {
		match File::options().write(true).create(true).open(path) {
			Ok(file) => Mutex::new(BufWriter::new(Box::new(file))),
			Err(e) => {
				panic!("Failed to open output file: {}", e);
			}
		}
	} else {
		Mutex::new(BufWriter::new(Box::new(std::io::stdout())))
	};

	let format = Format::from(&args.format);

	stream
		.lock()
		.unwrap()
		.write(match format {
			Format::Json => br#"[{"channelId":"i64","videoId":"i64","commentId":"string","commenterId":"i64","createdAt":"string","text":"string"}"#,
			Format::Csv => b"channel_id,video_id,comment_id,commenter_id,created_at,text",
		})
		.expect("Failed to write to output file");

	if args.channel.is_empty() {
		let videos = Video::paginate_filter(&http, &args.video);
		let mut chunked = videos.chunks(args.threads);

		while let Some(chunk) = chunked.next().await {
			futures::stream::iter(
				chunk
					.into_iter()
					.filter_map(|v| match v {
						Ok(v) => Some(Video::from(v)),
						Err(_) => None,
					})
					.map(|v| v.write_to_stream(&http, &stream, &format)),
			)
			.buffer_unordered(args.threads)
			.collect::<Vec<_>>()
			.await;
		}
	} else {
		let channels = futures::stream::iter(args.channel.into_iter().map(|c| {
			// TODO: fix this without leaking
			let c: &'static str = Box::leak(Box::from(c));

			Channel::from_username(&http, &c)
		}))
		.buffer_unordered(args.threads)
		.filter_map(|c| async move {
			if let Ok(Some(c)) = c {
				Some(c)
			} else {
				None
			}
		})
		.collect::<Vec<_>>()
		.await;

		let mut limit = args.limit.unwrap_or(0);
		let threads = args.threads;
		let (stream, channels) =
			use_writer_channels(&http, channels, &mut limit, threads, stream, &format).await;

		if args.live {
			let mut stream: Mutex<BufWriter<Box<dyn Write>>> = stream;
			let mut channels = channels;

			loop {
				tokio::time::sleep(std::time::Duration::from_secs_f64(args.wait * 60.)).await;

				(stream, channels) =
					use_writer_channels(&http, channels, &mut limit, threads, stream, &format)
						.await;
			}
		}

		let mut stream = stream.lock().unwrap();

		if format == Format::Json {
			stream.write(b"]").expect("Failed to write to output file");
		}

		stream.flush().expect("Failed to flush output file");
	}
}

async fn use_pg_channels(
	http: &reqwest::Client,
	mut channels: Vec<Channel>,
	limit: &mut usize,
	threads: usize,
	quiet: bool,
	client: tcd::prisma::PrismaClient,
	first: bool,
) -> (tcd::prisma::PrismaClient, Vec<Channel>) {
	for channel in channels.iter_mut() {
		let mut stop = false;
		let stop_at = channel.last_video_id.unwrap_or(0);

		let start_at = if first {
			match client
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
				Err(e) => panic!(
					"Failed to fetch latest video for {}: {}",
					channel.username, e
				),
			}
		} else {
			chrono::DateTime::<chrono::Utc>::MIN_UTC.with_timezone(&chrono::FixedOffset::east(0))
		};

		let mut videos = channel.paginate_mut(&http);

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
			if *limit > 0 {
				if videos.len() > *limit {
					videos.truncate(*limit);
					stop = true;
				}

				*limit -= videos.len();
			}

			let idx = videos.iter().position(|v| v.node.id == stop_at);

			if let Some(idx) = idx {
				videos.truncate(idx);
				stop = true;
			}

			let idx = videos.iter().position(|v| v.node.created_at < start_at);

			if let Some(idx) = idx {
				videos.drain(..idx);
				stop = true;
			}

			futures::stream::iter(videos.into_iter().map(|v| Video::from(v)).map(|v| async {
				v.save(&client).await.ok();
				v.write_to_pg(&http, &client, !quiet).await
			}))
			.buffer_unordered(threads)
			.collect::<Vec<_>>()
			.await;

			if stop {
				break;
			}
		}
	}

	(client, channels)
}

async fn use_pg(http: reqwest::Client, args: Args) {
	let client = match prisma::new_client().await {
		Ok(client) => client,
		Err(err) => panic!("Failed to connect to database: {}", err),
	};

	if args.channel.is_empty() {
		let videos = Video::paginate_filter(&http, &args.video);
		let mut chunked = videos.chunks(args.threads);

		while let Some(chunk) = chunked.next().await {
			futures::stream::iter(
				chunk
					.into_iter()
					.filter_map(|v| match v {
						Ok(v) => Some(Video::from(v)),
						Err(_) => None,
					})
					.map(|v| v.write_to_pg(&http, &client, !args.quiet)),
			)
			.buffer_unordered(args.threads)
			.collect::<Vec<_>>()
			.await;
		}
	} else {
		let channels = futures::stream::iter(args.channel.into_iter().map(|c| {
			// TODO: fix this without leaking
			let c: &'static str = Box::leak(Box::from(c));

			Channel::from_username(&http, &c)
		}))
		.buffer_unordered(args.threads)
		.filter_map(|c| async move {
			if let Ok(Some(c)) = c {
				Some(c)
			} else {
				None
			}
		})
		.collect::<Vec<_>>()
		.await;

		let mut limit = args.limit.unwrap_or(0);
		let threads = args.threads;

		let (client, channels) = use_pg_channels(
			&http, channels, &mut limit, threads, args.quiet, client, true,
		)
		.await;

		if args.live {
			let mut client: tcd::prisma::PrismaClient = client;
			let mut channels = channels;

			loop {
				tokio::time::sleep(std::time::Duration::from_secs_f64(args.wait * 60.)).await;

				(client, channels) = use_pg_channels(
					&http, channels, &mut limit, threads, args.quiet, client, false,
				)
				.await;
			}
		}
	}
}
