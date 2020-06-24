#!/usr/local/bin/dumb-init /bin/bash
set -e
echo "Иницализация контейнера '$BW_PROJ_NAME-proj'. . ."

cat <<USAGE 
Контейнер '$BW_PROJ_NAME-proj' инициализирован
Доступные команды см. в README.md
Нажмите CTRL+C
USAGE

exec dumb-init -- /bin/bash
