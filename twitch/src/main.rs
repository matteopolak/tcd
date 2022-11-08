#![feature(future_join)]
mod channel;
mod cli;
mod gql;
mod prisma;
mod video;

use std::future::join;

use clap::Parser;
use cli::Args;
use dotenv::dotenv;
use futures::StreamExt;

use crate::channel::{Channel, ChannelExt};

#[tokio::main]
async fn main() {
	dotenv().unwrap();

	let args = Args::parse();
	let client = match prisma::new_client().await {
		Ok(client) => client,
		Err(err) => panic!("Failed to connect to database: {}", err),
	};

	for channel_name in args.channel {
		let channel = match Channel::from_username(&channel_name).await {
			Ok(channel) => channel,
			Err(err) => {
				eprintln!("Failed to fetch channel {}: {}", channel_name, err);
				continue;
			}
		};

		channel.save(&client).await.unwrap();

		let mut video_iter = channel.get_videos();
		let mut video_batch = video_iter.batch();

		while let Some(videos) = video_batch.next().await {
			futures::stream::iter(videos.into_iter().map(|v| {
				let download_comments_future = v.get_comments().download_all(&client, args.verbose);
				let save_video_future = v.save(&client);

				join!(save_video_future, download_comments_future)
			}))
			.buffer_unordered(10)
			.collect::<Vec<_>>()
			.await;
		}
	}
}
