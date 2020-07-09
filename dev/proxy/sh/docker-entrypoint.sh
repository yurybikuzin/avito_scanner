#!/usr/bin/dumb-init bash
set -e
echo "Иницализация контейнера '$BW_PROJ_NAME-proxy'. . ."

sudo chown -R rust:rust \
    /home/rust/.cargo/git \
    /home/rust/.cargo/registry \
    /home/rust/src/target 

cat <<USAGE 
Контейнер '$BW_PROJ_NAME-proxy' инициализирован
Доступные команды см. в README.md
Нажмите CTRL+C
USAGE

exec bash
