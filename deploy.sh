#!/usr/bin/env bash
target_host=bikuzin18
target_dir=/home/bikuzin/avito

echo Creating $target_host:$target_dir . . .
ssh $target_host "mkdir -p $target_dir"
echo Stopping docker container . . .
ssh $target_host "cd $target_dir && docker-compose down"
echo Copiing prod/.env to $target_host:$target_dir/.env . . .
scp prod/.env $target_host:$target_dir/.env
echo Copiing prod/server-docker-compose.yml to $target_host:$target_dir/docker-compose.yml . . .
scp prod/server-docker-compose.yml $target_host:$target_dir/docker-compose.yml
echo Starting docker container . . .
ssh $target_host "cd $target_dir && docker-compose up -d"
echo Done
