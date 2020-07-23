#!/usr/bin/env bash
target_host=bikuzin18
target_dir=/home/bikuzin/proxy

echo Creating $target_host:$target_dir/cnf . . .
ssh $target_host "mkdir -p $target_dir/cnf"

echo Stopping docker container . . .
ssh $target_host "cd $target_dir && docker-compose down"

echo Copiing prod/.env to $target_host:$target_dir/.env . . .
scp prod/.env $target_host:$target_dir/.env

echo Copiing cnf/proxy/prod.toml to $target_host:$target_dir/cnf/prod.toml . . .
scp cnf/proxy/prod.toml $target_host:$target_dir/cnf/prod.toml

echo Copiing prod/proxy-docker-compose.yml to $target_host:$target_dir/docker-compose.yml . . .
scp prod/proxy-docker-compose.yml $target_host:$target_dir/docker-compose.yml

echo Copiing prod/logs.sh to $target_host:$target_dir/logs.sh . . .
scp prod/logs.sh $target_host:$target_dir/logs.sh

echo Starting docker container . . .
ssh $target_host "cd $target_dir && docker-compose up -d"

echo Done
