#!/usr/bin/env bash
set -e
docker-compose up -d
docker exec -it avito-proj cargo build --release 
cp dev/proj/cargo-target/release/scan prod/proj/copy/

