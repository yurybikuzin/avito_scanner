#!/usr/bin/env bash
target_host=bikuzin18
target_dir=/home/bikuzin/proxy

dir=$(dirname "$0")

echo pushd "$dir" . . .
pushd "$dir"

echo Creating $target_host:$target_dir/cnf . . .
ssh $target_host "mkdir -p $target_dir/cnf"

echo Stopping docker container . . .
ssh $target_host "cd $target_dir && docker-compose down"

echo docker system prune . . .
ssh $target_host "docker system prune --force"

echo docker volume prune . . .
ssh $target_host "docker volume prune --force"

echo Copiing .env to $target_host:$target_dir/.env . . .
scp .env $target_host:$target_dir/.env

echo Copiing cnf/proxy/prod.toml to $target_host:$target_dir/cnf/proxy.toml . . .
scp ../../cnf/proxy/prod.toml $target_host:$target_dir/cnf/proxy.toml

echo Copiing cnf/rmq/docker.toml to $target_host:$target_dir/cnf/rmq.toml . . .
scp ../../cnf/rmq/docker.toml $target_host:$target_dir/cnf/rmq.toml

echo Copiing prod/proxy/docker-compose.yml to $target_host:$target_dir/docker-compose.yml . . .
scp docker-compose.yml $target_host:$target_dir/docker-compose.yml

echo Copiing prod/proxy/logs.sh to $target_host:$target_dir/logs.sh . . .
scp logs.sh $target_host:$target_dir/logs.sh

echo Starting docker container . . .
ssh $target_host "cd $target_dir && docker-compose up -d"

echo Starting docker container . . .
ssh $target_host "cd $target_dir && docker-compose up -d && ./logs.sh"

echo Done

