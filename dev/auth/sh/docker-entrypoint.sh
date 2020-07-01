#!/usr/local/bin/dumb-init /bin/bash
set -e
echo "Иницализация контейнера '$BW_PROJ_NAME-auth'. . ."

cat <<USAGE 
Контейнер '$BW_PROJ_NAME-auth' инициализирован 
Доступные команды:
    curl localhost:$BW_DEV_AUTH_PORT -i
USAGE

exec dumb-init -- nodemon -- src/server.js
