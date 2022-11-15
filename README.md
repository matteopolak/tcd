# Twitch Chat Downloader 🗒️

![Build Status](https://github.com/matteopolak/tcd/actions/workflows/build.yml/badge.svg)
![Release Status](https://github.com/matteopolak/tcd/actions/workflows/release.yml/badge.svg)
[![License:GPLv3](https://img.shields.io/badge/license-GPL--3.0-yellow.svg)](https://opensource.org/licenses/GPL-3.0)
[![Rust:Nightly](https://img.shields.io/badge/rust-nightly-blue.svg)](https://www.rust-lang.org/tools/install)

[tcd](https://github.com/matteopolak/tcd) is a multi-threaded **T**witch **C**hat **D**ownloader built in Rust 🦀.

```powershell
Usage: tcd [OPTIONS] <--channel <CHANNEL>|--video <VIDEO>>

Options:
  -c, --channel <CHANNEL>      The channel(s) to download
  -i, --client-id <CLIENT_ID>  The Twitch client ID to use in the request headers
  -f, --format <FORMAT>        Used with --output or --stdout [default: csv] [possible values: json, csv]
  -l, --limit <LIMIT>          Downloads the first n videos from each channel
  -e, --live                   If specified, polls for new videos every `poll` seconds
  -o, --output <OUTPUT>        If specified, pipes data to the file
  -p, --postgres [<POSTGRES>]  The PostgreSQL connection string [default: DATABASE_URL env]
  -q, --quiet                  Whether to print download progress
  -s, --stdout                 If specified, pipes data to stdout
  -t, --threads <THREADS>      The number of threads to use [default: 10]
  -v, --video <VIDEO>          The video ids to download the chat for
  -w, --wait <WAIT>            The number of minutes to wait between polls (`live` only) [default: 30]
  -h, --help                   Print help information
  -V, --version                Print version information
```

Pipe the chat messages of the first 5 videos of `Atrioc`, `Linkus7` and `Aspecticor` to the file `hitman.csv`

```powershell
tcd --channel atrioc --channel linkus7 --channel aspecticor --limit 5 --output hitman.csv
```

Save the chat from the videos with id `1649326959` and `1648474855` to the connected PostgreSQL database.

```powershell
tcd --video 1649326959 --video 1648474855 --postgres
```

## Building from source

```bash
# build the binary
cargo build --release

# execute the binary
target/release/tcd --help
```

## Generating datasets

Some pre-made dataset scripts are located in the [queries](./queries) directory.
You can run these with `cargo run -p queries --example <name>`.

## Using pre-made datasets

Pre-made datasets can be downloaded from [the Mediafire folder](https://www.mediafire.com/folder/agnhlbxz0q5zw/datasets).

## Piping data to a database

`tcd` supports saving data directly to a PostgreSQL database.
First, apply the Prisma schema with the following commands:

```bash
# apply schema.prisma to the database
# note: this WILL wipe all database content
cargo prisma migrate dev --name init

# generate the Prisma client
cargo prisma generate
```

Or execute the [`migration.sql`](./scripts/migration.sql) SQL statements against your database.
Then, set the `DATABASE_URL` environment variable (a `.env` file works too), or supply the connection URL with `--postgres <url>`.

## Output format

Data piped to a file or stdout will be in the following format:

`--format csv`

```csv
channel_id,video_id,comment_id,commenter_id,created_at,text
23211159,1642642569,3f445ae2-2f6e-4256-b367-df8132454786,157032028,"2022-11-03 21:25:22.754 +00:00","poggies"
```

`--format json`

```json
[
  {
    "channelId": "i64",
    "videoId": "i64",
    "commentId": "string",
    "commenterId": "i64",
    "createdAt": "string",
    "text": "string"
  },
  {
    "channelId": 23211159,
    "videoId": 1642642569,
    "commentId": "3f445ae2-2f6e-4256-b367-df8132454786",
    "commenterId": 157032028,
    "createdAt": "2022-11-03 21:25:22.754 +00:00",
    "text": "poggies"
  }
]
```
