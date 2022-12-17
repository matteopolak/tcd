#![windows_subsystem = "windows"]
mod modal;

use std::fs::File;
use std::io::BufWriter;
use std::sync::{Arc, Mutex};

use crate::modal::Modal;
use futures::StreamExt;
use iced::alignment::{self, Alignment};
use iced::theme::{self, Theme};
use iced::widget::image::Handle;
use iced::widget::{
	button, column, container, row, scrollable, text, text_input, vertical_space, Image, Text,
};
use iced::{Application, Command, Element, Font, Length, Settings};
use once_cell::sync::Lazy;
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

pub fn main() -> iced::Result {
	App::run(Settings {
		default_font: Some(include_bytes!("../fonts/OpenSans.ttf")),
		..Settings::default()
	})
}

#[derive(Default)]
struct App {
	theme: Theme,
	search: String,
	filename: String,
	channel: Option<Result<Channel, ChannelError>>,
	videos: Option<Vec<Video>>,
	download_modal: Option<Video>,
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
	Downloaded,
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
	icon('\u{e258}')
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
					|_| Message::Downloaded,
				)
			}
			Message::Downloaded => Command::none(),
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
			// TODO: display videos in a grid with a width of 5 blocks and
			// an arbitrary height that is required to display all of them.
			// use a scrollbar container if the height is too big.
			(Some(Ok(_)), Some(videos)) => column(
				videos
					.chunks(3)
					.into_iter()
					.map(|videos| {
						row(videos
							.iter()
							.map(|video| {
								column![
									Image::new(Handle::from_memory(
										video.thumbnail.clone().unwrap()
									)),
									text(&video.title),
									button(download_icon())
										.on_press(Message::ShowModal(video.clone()))
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

		let content = row![
			row![choose_theme].align_items(Alignment::Start),
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
			.width(Length::Fill)
		]
		.padding(10)
		.width(Length::Fill)
		.height(Length::Fill);

		if let Some(video) = &self.download_modal {
			let modal = container(
				column![
					text("Download chat?"),
					text_input("Filename", &self.filename, Message::FilenameChanged),
					button("Go").on_press(Message::Download(video.clone()))
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
