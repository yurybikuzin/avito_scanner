#!/usr/bin/env bash

target_host=bikuzin
target_dir=/home/bikuzin/avito

dir=$(dirname "$0")

echo pushd "$dir" . . .
pushd "$dir"

echo Creating $target_host:$target_dir/cnf . . .
ssh $target_host "mkdir -p $target_dir/cnf"

echo Stopping docker container . . .
ssh $target_host "cd $target_dir && bash -c 'if [[ -f docker-compose.yml ]]; then docker-compose down --remove-orphans; fi'"

echo docker system prune . . .
ssh $target_host "docker system prune --force"

echo docker volume prune . . .
ssh $target_host "docker volume prune --force"

echo Copiing .env to $target_host:$target_dir/.env . . .
scp .env $target_host:$target_dir/.env

echo Copiing ../cnf/scan/config.toml to $target_host:$target_dir/cnf/config.toml . . .
scp ../cnf/scan/config.toml $target_host:$target_dir/cnf/config.toml

echo Copiing cnf/rmq/bikuzin18.toml to $target_host:$target_dir/cnf/bikuzin18.toml . . .
scp ../cnf/rmq/bikuzin18.toml $target_host:$target_dir/cnf/rmq.toml

echo Copiing docker-compose.yml to $target_host:$target_dir/docker-compose.yml . . .
scp docker-compose.yml $target_host:$target_dir/docker-compose.yml

# echo Copiing prod/logs.sh to $target_host:$target_dir/logs.sh . . .
# scp logs.sh $target_host:$target_dir/logs.sh

echo Starting docker container . . .
ssh $target_host "cd $target_dir && docker-compose up -d"

echo popd
popd

echo Done

