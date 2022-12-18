#![windows_subsystem = "windows"]
mod modal;

use std::fs::File;
use std::io::BufWriter;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use crate::modal::Modal;
use futures::StreamExt;
use iced::alignment::{self, Alignment};
use iced::theme::{self, Theme};
use iced::widget::image::Handle;
use iced::widget::{
	button, column, container, progress_bar, row, scrollable, text, text_input, vertical_space,
	Image, Text,
};
use iced::{Application, Command, Element, Font, Length, Settings};
use once_cell::sync::Lazy;
use std::collections::HashMap;
use tcd::channel::{Channel, ChannelError};
use tcd::gql::prelude::{ChunkError, PaginateMut, WriteChunk};
use tcd::video::Video;

static CLIENT_ID: &str = "kimne78kx3ncx6brgo4mv6wki5h1ko";
static HTTP: Lazy<reqwest::Client> = Lazy::new(|| {
	let mut headers = reqwest::header::HeaderMap::new();
	let client_id = std::env::var("CLIENT_ID");

	headers.insert(
		"Client-ID",
		reqwest::header::HeaderValue::from_str(if let Ok(client_id) = client_id.as_ref() {
			client_id
		// Otherwise, use the default client ID
		} else {
			CLIENT_ID
		})
		.expect("Invalid CLIENT_ID header value"),
	);

	reqwest::ClientBuilder::new()
		.default_headers(headers)
		.build()
		.expect("Failed to build HTTP client")
});
static NEXT_ID: AtomicU64 = AtomicU64::new(0);

pub fn main() -> iced::Result {
	App::run(Settings {
		default_font: Some(include_bytes!("../fonts/OpenSans.ttf")),
		..Settings::default()
	})
}

struct Task {
	video: Video,
	progress: Arc<Mutex<f32>>,
	id: u64,
}

#[derive(Default)]
struct App {
	theme: Theme,
	search: String,
	filename: String,
	channel: Option<Result<Channel, ChannelError>>,
	videos: Option<Vec<Video>>,
	download_modal: Option<Video>,
	tasks: HashMap<u64, Task>,
}

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
enum ThemeType {
	Light,
	Dark,
}

#[derive(Debug, Clone)]
enum Message {
	ThemeChanged(ThemeType),
	SearchChanged(String),
	FilenameChanged(String),
	Search,
	SearchResult(Result<Option<Channel>, ChannelError>),
	VideoResult(Result<Vec<Video>, ChunkError>),
	Download(Video),
	Downloaded(u64),
	TaskRemoved(u64),
	ShowModal(Video),
	CloseModal,
}

const ICONS: Font = Font::External {
	name: "Icons",
	bytes: include_bytes!("../fonts/icons.ttf"),
};

fn icon(unicode: char) -> Text<'static> {
	text(unicode.to_string())
		.font(ICONS)
		.width(Length::Units(20))
		.horizontal_alignment(alignment::Horizontal::Center)
		.vertical_alignment(alignment::Vertical::Center)
		.size(20)
}

fn dark_icon() -> Text<'static> {
	icon('\u{e1ad}')
}

fn light_icon() -> Text<'static> {
	icon('\u{e1ac}')
}

fn download_icon() -> Text<'static> {
	icon('\u{e2c4}')
}

fn delete_icon() -> Text<'static> {
	icon('\u{e872}')
}

impl Application for App {
	type Message = Message;
	type Executor = iced::executor::Default;
	type Flags = ();
	type Theme = theme::Theme;

	fn new(_flags: ()) -> (Self, Command<Self::Message>) {
		(App::default(), Command::none())
	}

	fn title(&self) -> String {
		format!("Twitch Chat Downloader v{}", env!("CARGO_PKG_VERSION"))
	}

	fn update(&mut self, message: Message) -> Command<Self::Message> {
		match message {
			Message::ThemeChanged(theme) => {
				self.theme = match theme {
					ThemeType::Light => Theme::Light,
					ThemeType::Dark => Theme::Dark,
				};

				Command::none()
			}
			Message::SearchChanged(search) => {
				self.search = search;

				Command::none()
			}
			Message::Search => Command::perform(
				tcd::channel::Channel::from_username(&HTTP, self.search.clone()),
				Message::SearchResult,
			),
			Message::SearchResult(channel) => {
				self.channel = channel.transpose();

				if let Some(Ok(mut channel)) = self.channel.clone() {
					Command::perform(
						async move {
							let data = channel.paginate_mut(&HTTP).collect::<Vec<_>>().await;
							let videos = data
								.into_iter()
								.flat_map(|container| container.edges.into_iter().map(Video::from))
								.collect::<Vec<Video>>();

							let videos = futures::stream::iter(videos)
								.map(|mut video| async move {
									video.get_thumbnail(&HTTP).await.ok();
									video
								})
								.buffer_unordered(10)
								.collect::<Vec<_>>()
								.await;

							Ok(videos)
						},
						Message::VideoResult,
					)
				} else {
					Command::none()
				}
			}
			Message::VideoResult(videos) => {
				self.videos = videos.ok();

				Command::none()
			}
			Message::Download(video) => {
				let filename = self.filename.clone();
				let task_id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
				let task = Task {
					video: video.clone_without_thumbnail(),
					progress: Arc::new(Mutex::new(0.0)),
					id: task_id,
				};

				self.tasks.insert(task.id, task);

				Command::perform(
					async move {
						let video = video;
						let stream = match File::options().write(true).create(true).open(filename) {
							Ok(file) => Arc::new(Mutex::new(BufWriter::new(Box::new(file)))),
							Err(e) => {
								panic!("Failed to open output file: {e}");
							}
						};

						video
							.write_to_stream(&HTTP, &stream, &tcd::gql::prelude::Format::Csv)
							.await
					},
					move |_| Message::Downloaded(task_id),
				)
			}
			Message::Downloaded(task_id) => {
				self.tasks.get_mut(&task_id).and_then(|task| {
					task.progress.lock().ok().map(|mut progress| {
						*progress = 1.0;
					})
				});

				Command::none()
			}
			Message::TaskRemoved(task_id) => {
				self.tasks.remove(&task_id);

				Command::none()
			}
			Message::ShowModal(video) => {
				self.download_modal = Some(video);

				Command::none()
			}
			Message::CloseModal => {
				self.download_modal = None;

				Command::none()
			}
			Message::FilenameChanged(filename) => {
				self.filename = filename;

				Command::none()
			}
		}
	}

	fn view(&self) -> Element<Message> {
		let choose_theme = match self.theme {
			Theme::Light => button(dark_icon()).on_press(Message::ThemeChanged(ThemeType::Dark)),
			Theme::Dark => button(light_icon()).on_press(Message::ThemeChanged(ThemeType::Light)),
			Theme::Custom(_) => unreachable!(),
		}
		.padding(10)
		.width(Length::Units(50))
		.height(Length::Units(50));

		let search_input = text_input(
			"Search for a streamer...",
			&self.search,
			Message::SearchChanged,
		);
		let search_button = button("Go").on_press(Message::Search);
		let channel = match (&self.channel, &self.videos) {
			(Some(Ok(_)), Some(videos)) => column(
				videos
					.chunks(3)
					.into_iter()
					.map(|videos| {
						row(videos
							.iter()
							.map(move |video| {
								column![
									Image::new(Handle::from_memory(
										video.thumbnail.clone().unwrap()
									)),
									text(&video.title),
									button(download_icon()).on_press(Message::ShowModal(
										video.clone_without_thumbnail()
									))
								]
								.width(Length::Fill)
								.into()
							})
							.collect())
						.padding(10)
						.into()
					})
					.collect(),
			),
			(Some(Err(_)), _) => column![text("An error occurred. Please try again.")],
			(None, _) | (Some(Ok(_)), None) => column![text("There's nothing here.")],
		};

		let tasks = scrollable(column(
			self.tasks
				.iter()
				.map(|task| {
					let task = task.1;
					let progress = task.progress.lock().unwrap();
					let progress = *progress;

					column![
						text(&task.video.title),
						progress_bar(0.0..=1.0, progress),
						if progress < 1.0 {
							button(delete_icon())
						} else {
							button(delete_icon()).on_press(Message::TaskRemoved(task.id))
						}
					]
					.padding(10)
					.into()
				})
				.collect(),
		));

		let content = row![
			column![choose_theme, tasks]
				.align_items(Alignment::Start)
				.width(Length::FillPortion(20))
				.max_width(300),
			column![
				container(row![search_input.width(Length::Units(300)), search_button])
					.align_x(iced::alignment::Horizontal::Center)
					.width(Length::Fill),
				vertical_space(Length::Units(10)),
				container(scrollable(channel))
					.align_y(iced::alignment::Vertical::Center)
					.align_x(iced::alignment::Horizontal::Center)
					.width(Length::Fill)
					.height(Length::Fill)
			]
			.width(Length::FillPortion(80))
		]
		.padding(10)
		.width(Length::Fill)
		.height(Length::Fill);

		if let Some(video) = &self.download_modal {
			let modal = container(
				column![
					text("Download chat?"),
					text_input("Filename", &self.filename, Message::FilenameChanged),
					button("Go").on_press(Message::Download(video.clone_without_thumbnail()))
				]
				.width(Length::Fill)
				.height(Length::Fill),
			)
			.width(Length::Units(300))
			.padding(10)
			.style(theme::Container::Box);

			Modal::new(content, modal)
				.on_blur(Message::CloseModal)
				.into()
		} else {
			content.into()
		}
	}

	fn theme(&self) -> Theme {
		self.theme.clone()
	}
}
