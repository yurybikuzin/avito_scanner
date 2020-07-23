#!/usr/bin/dumb-init bash
# set -e
# echo "Иницализация контейнера 'prod-$BW_PROJ_NAME-proxy'. . ."
#
# # chown -R $(whoami) /data 
#
# cat <<USAGE 
# Контейнер '$BW_PROJ_NAME-proj' инициализирован
# Доступные команды см. в README.md
# Нажмите CTRL+C
# USAGE

./proxy $BW_CONFIG
