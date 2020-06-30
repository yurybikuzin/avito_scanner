# Avito scanner (Rust and NodeJS::Puppeeteer)

<!-- vim-markdown-toc Redcarpet -->

* [Development](#development)
    * [Prerequisites](#prerequisites)
        * [Git](#git)
        * [Docker](#docker)
        * [Docker-Compose](#docker-compose)
    * [Prepare workplace](#prepare-workplace)
        * [Available commands](#available-commands)
            * [cargo](#cargo)
    * [Do not forget to stop docker containers after work is over](#do-not-forget-to-stop-docker-containers-after-work-is-over)
* [Files](#files)
* [Fairplay](#fairplay)

<!-- vim-markdown-toc -->

## Development

### Prerequisites

#### Git

https://git-scm.com/downloads

#### Docker

https://docs.docker.com/install

Pay attention to [Post-installation steps for Linux](https://docs.docker.com/engine/install/linux-postinstall/)

#### Docker-Compose

https://docs.docker.com/compose/install

### Prepare workplace

- clone repo: 

```
git clone git@github.com:yurybikuzin/avito_scanner.git
```

- up dev container: 

```
docker-compose up -d 
```

#### Available commands

##### cargo

```
docker exec -it avito-proj cargo
docker exec -it -e RUST_LOG=info avito-proj cargo test -p diaps
docker exec -it avito-proj cargo run
```

### Do not forget to stop docker containers after work is over

```
docker-compose down
```

## Files

- `README.md` - this is it
- `.gitignore` - see https://git-scm.com/docs/gitignore
- `.gitlab-ci.yml` - required for proper work of `docker/refresh.sh`
- `docker-compose.yml` - required for `docker-compose up -d`
- `.env` - required for `docker-compose up -d`
- `docker/refresh.sh` - tool for rebuilding docker container for service from `docker-compose.yml` (`proj` by default)
- `docker/proj/Dockerfile` - Dockerfile for service `proj`, mentioned in `docker-compose.yml`
- `docker/proj/sh/*` - helper sh-scripts used in `avito-proj` docker container
- `docker/auth/Dockerfile/` - Dockerfile for service `auth`, mentioned in `docker-compose.yml`
- `docker/auth/sh/*` - helper sh-scripts used in `avito-auth` docker container
- `docker/auth/src/*` - neccessary files for building docker container of service `auth`
- `Cargo.toml`, `Cargo.lock` - [cargo](https://doc.rust-lang.org/cargo/) files
- `diaps/*`, `ids/*`, `cards/*`, `scanner/*` - scanner source files, written in [Rust](https://www.rust-lang.org/)
- `out/*` - scanner output files

## Fairplay

https://vimeo.com/user58195081/review/394860047/b827eafd0d
23:43-24:18

