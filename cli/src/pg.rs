use crate::cli::Args;
use futures::StreamExt;
use prisma_client_rust::Direction;
use tcd::{
	channel::Channel,
	gql::prelude::{PaginateFilter, PaginateMut, Save, WriteChunk},
	prisma,
	video::Video,
};

async fn run_channels(
	http: &reqwest::Client,
	mut channels: Vec<Channel>,
	limit: &mut usize,
	threads: usize,
	quiet: bool,
	client: tcd::prisma::PrismaClient,
	first: bool,
) -> (tcd::prisma::PrismaClient, Vec<Channel>) {
	for channel in &mut channels {
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
				.map(|mut v| {
					if v.is_empty() {
						None
					} else {
						Some(v.remove(0))
					}
				}) {
				Ok(Some(video)) => video.created_at,
				Ok(None) => chrono::DateTime::<chrono::Utc>::MIN_UTC
					.with_timezone(&chrono::FixedOffset::east(0)),
				Err(e) => panic!("Failed to fetch latest video for {}: {e}", channel.username),
			}
		} else {
			chrono::DateTime::<chrono::Utc>::MIN_UTC.with_timezone(&chrono::FixedOffset::east(0))
		};

		let mut videos = channel.paginate_mut(http);

		while let Some(container) = videos.next().await {
			let container = match container {
				Ok(container) => container,
				Err(e) => {
					eprintln!("Failed to fetch videos: {e:?}");
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

			futures::stream::iter(videos.into_iter().map(Video::from).map(|v| async {
				v.save(&client).await.ok();
				v.write_to_pg(http, &client, !quiet).await
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

pub async fn run(http: reqwest::Client, args: Args) {
	let client = match prisma::new_client().await {
		Ok(client) => client,
		Err(e) => panic!("Failed to connect to database: {e}"),
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

		let (client, channels) = run_channels(
			&http, channels, &mut limit, threads, args.quiet, client, true,
		)
		.await;

		if args.live {
			let mut client: tcd::prisma::PrismaClient = client;
			let mut channels = channels;

			loop {
				tokio::time::sleep(std::time::Duration::from_secs_f64(args.wait * 60.)).await;

				(client, channels) = run_channels(
					&http, channels, &mut limit, threads, args.quiet, client, false,
				)
				.await;
			}
		}
	}
}
