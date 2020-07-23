#!/usr/bin/env bash
docker-compose -f proxy-docker-compose.yml down \
&& \
BW_HOST_CNF_DIR=../cnf \
BW_PROD_CONFIG=/cnf/proxy/local.toml \
docker-compose \
    -f proxy-docker-compose.yml \
    up -d \
&& \
docker logs prod-avito-proxy -f \
&& \
true

