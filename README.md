# Twitch Chat Downloader 🗒️

![Build Status](https://github.com/matteopolak/tcd/actions/workflows/rust.yml/badge.svg)
[![License:GPLv3](https://img.shields.io/badge/license-GPL--3.0-yellow.svg)](https://opensource.org/licenses/GPL-3.0)
[![Rust:Nightly](https://img.shields.io/badge/rust-nightly-blue.svg)](https://www.rust-lang.org/tools/install)

[tcd](https://github.com/matteopolak/tcd) is a multi-threaded **T**witch **C**hat **D**ownloader built in Rust 🦀.

## Setup

You will need a [PostgreSQL](https://www.postgresql.org/download "You can download it from here") database to store chat messages.

Rename `.env.example` to `.env` and replace the placeholder values with your own.

## Usage

```cli
tcd --channel <CHANNEL> [--quiet]
```

For example:

```cli
tcd -c atrioc -c linkus7 -c aspecticor
```

## Building from source

```bash
# apply schema.prisma to the database
# note: this WILL wipe all database content
cargo prisma migrate dev --name init

# generate the Prisma client
cargo prisma generate

# build the binary
cargo build --release

# execute the binary
target/release/tcd -c atrioc
```

## Generating datasets

Some pre-made dataset scripts are located in the [datasets](./datasets) directory.
You can run these with `cargo run -p datasets --example <name>`.
