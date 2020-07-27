#!/usr/bin/env bash
docker-compose down --remove-orphans\
&& \
BW_HOST_CNF_DIR=../../cnf \
BW_PROD_CONFIG=/cnf/proxy/local.toml \
BW_PROD_CONFIG_RMQ=/cnf/rmq/docker.toml \
docker-compose \
    up -d \
&& \
docker logs prod-avito-proxy -f \
&& \
true

