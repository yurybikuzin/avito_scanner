#!/usr/bin/env bash

dir=$(dirname "$0")
pushd "$dir"

docker-compose down --remove-orphans\
&& \
BW_HOST_CNF_DIR=../cnf \
docker-compose \
    up -d \
&& \
docker exec -it prod-avito-proj scan /cnf/scan/config.toml --rmq /cnf/rmq/bikuzin18.toml \
&& \
true

popd

