use crate::cli::Args;
use futures::StreamExt;
use std::{
	fs::File,
	io::{BufWriter, Write},
	sync::Mutex,
};
use tcd::{
	channel::Channel,
	gql::prelude::{Format, PaginateFilter, PaginateMut, WriteChunk},
	video::Video,
};

async fn run_channels(
	http: &reqwest::Client,
	mut channels: Vec<Channel>,
	limit: &mut usize,
	threads: usize,
	stream: Mutex<BufWriter<Box<dyn Write + Send>>>,
	format: &Format,
) -> (Mutex<BufWriter<Box<dyn Write + Send>>>, Vec<Channel>) {
	for channel in &mut channels {
		let mut stop = false;
		let stop_at = channel.last_video_id.unwrap_or(0);
		let mut videos = channel.paginate_mut(http);

		while let Some(container) = videos.next().await {
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
					.map(Video::from)
					.map(|v| v.write_to_stream(http, &stream, format)),
			)
			.buffer_unordered(threads)
			.collect::<Vec<_>>()
			.await;

			if stop {
				break;
			}
		}
	}

	stream
		.lock()
		.unwrap()
		.flush()
		.expect("Failed to flush output file");

	(stream, channels)
}

pub async fn run(http: reqwest::Client, mut args: Args) {
	// Suppress logs when writing to a file or stdout
	args.quiet = true;

	let stream: Mutex<BufWriter<Box<dyn Write + Send>>> = if let Some(path) = &args.output {
		match File::options().write(true).create(true).open(path) {
			Ok(file) => Mutex::new(BufWriter::new(Box::new(file))),
			Err(e) => {
				panic!("Failed to open output file: {e}");
			}
		}
	} else {
		Mutex::new(BufWriter::new(Box::new(std::io::stdout())))
	};

	let format = Format::from(&args.format);

	if format == Format::Csv {
		stream
			.lock()
			.unwrap()
			.write_all(b"channel,video_id,comment_id,commenter,created_at,text\n")
			.expect("Failed to write to output file");
	}

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

			Channel::from_username(&http, c)
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
			run_channels(&http, channels, &mut limit, threads, stream, &format).await;

		if args.live {
			let mut stream: Mutex<BufWriter<Box<dyn Write + Send>>> = stream;
			let mut channels = channels;

			loop {
				tokio::time::sleep(std::time::Duration::from_secs_f64(args.wait * 60.)).await;

				(stream, channels) =
					run_channels(&http, channels, &mut limit, threads, stream, &format).await;
			}
		}

		let mut stream = stream.lock().unwrap();

		stream.flush().expect("Failed to flush output file");
	}
}
