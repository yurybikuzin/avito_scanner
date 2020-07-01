# Сканер Авито (Rust and NodeJS::Puppeeteer)

<!-- vim-markdown-toc Redcarpet -->

* [О программе](#о-программе)
* [Подготовка рабочего места](#подготовка-рабочего-места)
    * [Требуемые программы](#требуемые-программы)
        * [Git](#git)
        * [Docker](#docker)
        * [Docker-Compose](#docker-compose)
    * [Репозиторий](#репозиторий)
* [Использование](#использование)
    * [Быстро и просто](#быстро-и-просто)
            * [Завершение работы](#завершение-работы)
    * [Долго и надежно](#долго-и-надежно)
            * [Завершение работы](#завершение-работы)
* [Разработка](#разработка)
    * [Подготовка](#подготовка)
    * [Доступные команды](#доступные-команды)
        * [Получение *AVITO_AUTH*](#получение-avito_auth)
        * [cargo check](#cargo-check)
        * [cargo test](#cargo-test)
        * [cargo run](#cargo-run)
        * [Завершение работы](#завершение-работы)
* [Процедура сборки](#процедура-сборки)
* [Files](#files)
* [Fairplay](#fairplay)

<!-- vim-markdown-toc -->

## О программе

Сканер объявлений c Авито из раздела Автомобили

## Подготовка рабочего места

Необходимо убедиться, что установлены, и при отсутствии установить следующие программы:

### Требуемые программы

#### Git

https://git-scm.com/downloads

#### Docker

https://docs.docker.com/install

Если у Вас Linux, то настоятельно рекомендуется выполнить [Post-installation steps for Linux](https://docs.docker.com/engine/install/linux-postinstall/), чтобы выполнять команды `docker` и `docker-compose` без `sudo`

#### Docker-Compose

https://docs.docker.com/compose/install

### Репозиторий

Также необходимо развернуть репозиторий проекта в папку `avito`:

```
git clone git@github.com:yurybikuzin/avito_scanner.git avito
```

и перейти в корневую папку проекта:

```
cd avito
```

Все следующие команды следует выполнять из корневой папки проекта

## Использование

### Быстро и просто

В этом случае будет скачана программа размером ~21.3MB (мегабайт)

```
AVITO_AUTH=af0deccbgcgidddjgnvljitntccdduijhdinfgjgfjir docker-compose -f simple-prod-docker-compose.yml up -d
docker exec -it prod-avito-proj scan
```

Результат работы программы будет помещен в папку `out`

##### Завершение работы

```
docker-compose -f simple-prod-docker-compose.yml down
```

### Долго и надежно

В случае, если ключ авторизации (`AVITO_AUTH`) не подойдет, придется скачать программу размером ~1.35GB (гигабайт!)

```
docker-compose -f full-prod-docker-compose.yml up -d
docker exec -it prod-avito-proj scan
```

##### Завершение работы

```
docker-compose -f full-prod-docker-compose.yml down
```

## Разработка

### Подготовка

Сначала необходимо *поднять* docker-container'ы для разработки: 

```
docker-compose up -d 
```

### Доступные команды

#### Получение *AVITO_AUTH*

```
curl localhost:42001
```

#### cargo check

```
docker exec -it avito-proj cargo check
```

#### cargo test

```
docker exec -it -e AVITO_AUTH=af0deccbgcgidddjgnvljitntccdduijhdinfgjgfjir -e RUST_LOG=info avito-proj cargo test -p diaps
```

#### cargo run

```
docker exec -it avito-proj cargo run
```


#### Завершение работы

```
docker-compose down
```

## Процедура сборки

Процедура заключается в независимой сборке *docker image* для двух сервисов: `proj` и `auth`

Перед сборкой необходимо проверить файлы `.env` и `prod.yml`
Версия сервиса указанная в файле `.env` (*BW_PROD_СЕРВИС_VERSION*) должна быть больше версии, указаной в файле `prod.yml`

Общий вид команды сборки: ```./docker-image.sh prod СЕРВИС```

```
./docker-image.sh prod proj
и/или
./docker-image.sh prod auth
```

В результате сборки будет создан *docker image* `bazawinner/prod-avito-СЕРВИС:ВЕРСИЯ` и помещен на https://hub.docker.com/ (чтобы получить доступ к https://hub.docker.com/ необходимо авторизоваться: ```docker login```)

После успешной сборки необходимо отразить в файле `prod.yml` версию, указанную в файле `.env` (BW_PROD_СЕРВИС_VERSION)

## Files

- `README.md` - этот файл
- `.gitignore` - см. https://git-scm.com/docs/gitignore
- `docker-compose.yml` - docker-compose файл для разработки
- `simple-prod-docker-compose.yml` - docker-compose файл для быстрого и простого запуска сканера в production
- `full-prod-docker-compose.yml` - docker-compose файл для долгого и полного запуска сканера в production
- `.env` - используется командой `docker-compose`
- `cargo-git`, `cargo-target`, `cargo-registry` - кеш контейнера avito-proj (для разработки)
- `docker-image.sh` - инструмент для сборки *docker image*
- `dev/` - файлы, необходимые для создания *docker image* контейнеров для разработки
- `prod/` - файлы, необходимые для создания *docker image* контейнеров для production
- `dev.yml`, `prod.yml` - файлы версий *docker image* контейнеров для разработки и production
- `Cargo.toml`, `Cargo.lock` - [cargo](https://doc.rust-lang.org/cargo/) файлы
- `out/` - результаты работы сканера
- `arrange_millis`, `auth/`, `cards/`, `diap_store/`, `diaps/`, `env/`, `id_store`, `ids/`, `scan/`, `term` - source files, written in [Rust](https://www.rust-lang.org/)

## Fairplay

https://vimeo.com/user58195081/review/394860047/b827eafd0d
23:43-24:18

