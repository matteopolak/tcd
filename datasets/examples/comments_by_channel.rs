use dotenv::dotenv;
use prisma_client_rust::{raw, QueryError};
use serde::{Deserialize, Serialize};
use tcd::prisma;

use csv::Writer;

#[derive(Deserialize, Serialize, Debug)]
struct QueryReturnType {
	channel: String,
	comments: i64,
}

#[tokio::main]
async fn main() -> Result<(), QueryError> {
	dotenv().unwrap();

	let client = match prisma::new_client().await {
		Ok(client) => client,
		Err(err) => panic!("Failed to connect to database: {}", err),
	};

	let data: Vec<QueryReturnType> = client
		._query_raw(raw!(
			r#"
				SELECT "User"."username" AS "channel", COUNT(*) AS "comments"
					FROM "Comment"
					LEFT JOIN "Video"
					ON "Video"."id" = "Comment"."videoId"
					LEFT JOIN "User"
					ON "Video"."authorId" = "User"."id"
					GROUP BY "User"."username"
					ORDER BY COUNT(*) DESC
					LIMIT 100;
			"#
		))
		.exec()
		.await?;

	let mut wtr = Writer::from_writer(std::io::stdout());

	for row in data {
		wtr.serialize(row).unwrap();
	}

	wtr.flush().unwrap();

	Ok(())
}
