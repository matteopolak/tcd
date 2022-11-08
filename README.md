# Twitch Chat Downloader 🗒️

![Build Status](https://github.com/matteopolak/tcd/actions/workflows/rust.yml/badge.svg)
[![License:MIT](https://img.shields.io/badge/license-MIT-yellow.svg)](https://opensource.org/licenses/MIT)
[![Rust:Nightly](https://img.shields.io/badge/rust-nightly-blue.svg)](https://www.rust-lang.org/tools/install)

[tcd](https://github.com/matteopolak/tcd) is a multi-threaded **T**witch **C**hat **D**ownloader built in Rust 🦀.

## Usage

```cli
tcd --channel <CHANNEL>
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
