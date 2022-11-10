# Twitch Chat Downloader 🗒️

![Build Status](https://github.com/matteopolak/tcd/actions/workflows/rust.yml/badge.svg)
[![License:GPLv3](https://img.shields.io/badge/license-GPL--3.0-yellow.svg)](https://opensource.org/licenses/GPL-3.0)
[![Rust:Nightly](https://img.shields.io/badge/rust-nightly-blue.svg)](https://www.rust-lang.org/tools/install)

[tcd](https://github.com/matteopolak/tcd) is a multi-threaded **T**witch **C**hat **D**ownloader built in Rust 🦀.

```powershell
Usage: tcd [OPTIONS] --channel <CHANNEL>

Options:
  -c, --channel <CHANNEL>      The channel to download. Specify multiple times to download multiple channels
  -t, --threads <THREADS>      The number of threads to use [default: 10]
  -q, --quiet                  Whether to print download progress Always true if --output or --stdout is specified
  -s, --stdout                 Whether to pipe data to stdout Overridden by --output and --postgres
  -l, --limit <LIMIT>          Downloads the first n videos from each channel
  -o, --output <OUTPUT>        The file to pipe data to If not specified, data will be printed to stdout Overridden by --postgres
  -p, --postgres <POSTGRES>    The PostgreSQL connection string This will take precedence over all other output arguments
  -i, --client-id <CLIENT_ID>  The Twitch client ID to use in the request headers If not specified, the CLIENT_ID environment variable will be used if it exists, otherwise the default client ID will be used
  -h, --help                   Print help information
  -V, --version                Print version information
```

Pipe the chat messages of the first 5 videos of `Atrioc`, `Linkus7` and `Aspecticor` to the file `hitman.csv`

```cli
tcd -c atrioc -c linkus7 -c aspecticor -o hitman.csv --limit 5
```

## Output format

Data piped to a file or stdout will be in the following format:

```csv
channel_id,video_id,comment_id,commenter_id,created_at,text
23211159,1642642569,3f445ae2-2f6e-4256-b367-df8132454786,157032028,"2022-11-03 21:25:22.754 +00:00","poggies"
23211159,1642642569,da21e286-7c53-461f-b74f-a41f8a6b5c31,157032028,"2022-11-03 21:25:31.685 +00:00","going live"
23211159,1642642569,8bf921c1-6606-4a5a-8703-0a182c20689e,57036924,"2022-11-03 21:25:32.919 +00:00","peepoHey big a"
```

## Building from source

```bash
# build the binary
cargo build --release

# execute the binary
target/release/tcd -c atrioc
```

## Generating datasets

Some pre-made dataset scripts are located in the [datasets](./datasets) directory.
You can run these with `cargo run -p datasets --example <name>`.

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

Then, set the `DATABASE_URL` environment variable (a `.env` file works too), or supply the connection URL with `--postgres <url>`.
