#!/usr/bin/env bash
set -e
docker-compose up -d
# docker exec -it avito-proj rustup target add x86_64-unknown-linux-musl
# docker exec -it avito-proj cargo build --release --target=x86_64-unknown-linux-musl
# cp dev/proj/cargo-target/x86_64-unknown-linux-musl/release/scan prod/proj/copy/
docker exec -it avito-proj cargo build --release 
cp dev/proj/cargo-target/release/scan prod/proj/copy/

