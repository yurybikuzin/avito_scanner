#!/usr/bin/env bash
set -e
docker-compose up -d
docker exec -it avito-proxy cargo build -p proxy --release --target=x86_64-unknown-linux-musl
cp dev/proj/cargo-target/x86_64-unknown-linux-musl/release/proxy prod/proxy/copy/

