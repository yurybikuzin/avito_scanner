#!/usr/bin/env bash
target_host=bikuzin18
target_dir=/home/bikuzin/avito

echo Creating $target_host:$target_dir . . .
ssh $target_host "mkdir -p $target_dir"
echo Stopping docker container . . .
ssh $target_host "cd $target_dir && docker-compose -f server-prod-docker-compose.yml down"
echo Copiing .env server-prod-docker-compose.yml . . .
scp .env prod-docker-compose.yml $target_host:$target_dir
echo Starting docker container . . .
ssh $target_host "cd $target_dir && docker-compose -f prod-docker-compose.yml up -d"
echo Done
