#!/usr/bin/env bash
set -e
# docker run \
#     --rm \
#     -v \
#     "$(pwd)":/home/rust/src ekidd/rust-musl-builder \
#     cargo build --release
# mv scan prod/proj/copy
# docker run -it \
#     --mount type=bind,source="$(pwd)",target=/proj \
#     --mount type=bind,source="$(pwd)/registry",target=/usr/local/cargo/registry \
#     -w /proj \
#     bazawinner/dev-avito-proj:$BW_DEV_PROJ_VERSION \
#     cargo build --release --target=x86_64-unknown-linux-musl
# --features=simd
docker-compose up -d
# docker exec -it avito-proj rustup target add x86_64-unknown-linux-musl
docker exec -it avito-proj cargo build --release --target=x86_64-unknown-linux-musl
cp dev/proj/cargo-target/x86_64-unknown-linux-musl/release/scan prod/proj/copy/

