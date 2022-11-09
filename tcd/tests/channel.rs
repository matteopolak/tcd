use tcd::channel::Channel;

static CLIENT_ID: &str = "kimne78kx3ncx6brgo4mv6wki5h1ko";

#[tokio::test]
async fn test_valid_channel() {
	std::env::set_var("CLIENT_ID", CLIENT_ID);

	let channel = Channel::from_username("atrioc").await.unwrap();

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
	std::env::set_var("CLIENT_ID", CLIENT_ID);

	let channel = Channel::from_username("_").await.unwrap();

	assert_eq!(None, channel);
}
