use tcd::channel::Channel;

static CLIENT_ID: &str = "kimne78kx3ncx6brgo4mv6wki5h1ko";

fn get_client() -> reqwest::Client {
	reqwest::Client::builder()
		.user_agent("tcd")
		.default_headers({
			let mut headers = reqwest::header::HeaderMap::new();
			headers.insert(
				"Client-ID",
				reqwest::header::HeaderValue::from_static(CLIENT_ID),
			);
			headers
		})
		.build()
		.unwrap()
}

#[tokio::test]
async fn test_valid_channel() {
	let client = get_client();
	let channel = Channel::from_username(&client, "atrioc").await.unwrap();

	assert_eq!(
		Some(Channel {
			id: 23211159,
			username: "atrioc".to_string()
		}),
		channel
	);
}

#[tokio::test]
async fn test_invalid_channel() {
	let client = get_client();
	let channel = Channel::from_username(&client, "_").await.unwrap();

	assert_eq!(None, channel);
}
