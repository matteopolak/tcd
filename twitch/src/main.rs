#![feature(future_join)]
mod channel;
mod gql;
mod prisma;
mod video;

use std::future::join;

use dotenv::dotenv;
use futures::StreamExt;

use crate::channel::channel::{Channel, ChannelExt};

#[tokio::main]
async fn main() {
	dotenv().unwrap();

	let client = prisma::new_client().await.unwrap();
	let channel = Channel::from_username("xqc".to_string()).await.unwrap();

	channel.save(&client).await.unwrap();

	let mut video_iter = channel.get_videos();
	let mut video_batch = video_iter.batch();

	while let Some(videos) = video_batch.next().await {
		let start = std::time::Instant::now();

		futures::stream::iter(videos.into_iter().map(|v| {
			let download_comments_future = v.get_comments().download_all(&client);
			let save_video_future = v.save(&client);

			join!(save_video_future, download_comments_future)
		}))
		.buffer_unordered(10)
		.collect::<Vec<_>>()
		.await;

		println!("Took {:?}", start.elapsed().as_millis());
	}
}
