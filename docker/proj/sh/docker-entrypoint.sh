#!/usr/local/bin/dumb-init /bin/bash
set -e
echo "Иницализация контейнера '$BW_PROJ_NAME-proj'. . ."

cat <<USAGE 
== USAGE >>
Проект '$BW_PROJ_NAME' инициализирован
Доступные команды см. в README.md
<< USAGE ==
Нажмите CTRL+C
USAGE

exec dumb-init -- /bin/bash
