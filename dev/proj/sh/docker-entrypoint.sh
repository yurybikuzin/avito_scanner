#!/usr/bin/dumb-init bash
set -e
echo "Иницализация контейнера '$BW_PROJ_NAME-proj'. . ."

sudo chown -R rust:rust \
    /home/rust/.cargo/git \
    /home/rust/.cargo/registry \
    /home/rust/src/target \
    /out

cat <<USAGE 
Контейнер '$BW_PROJ_NAME-proj' инициализирован
Доступные команды см. в README.md
Нажмите CTRL+C
USAGE

exec dumb-init -- bash
