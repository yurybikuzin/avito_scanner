# Echo-server (Rust/Rocket) 
<!-- vim-markdown-toc Redcarpet -->

* [About](#about)
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

<!-- vim-markdown-toc -->

## About

simple echo-server for proxy-checker

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
git clone git@github.com:yurybikuzin/echo_server.git
```

- up dev container: 

```
docker-compose up -d 
```

#### Available commands

##### cargo

```
docker exec -it echo-proj cargo
docker exec -it -e RUST_LOG=info echo-proj cargo test -p echo
docker exec -it echo-proj cargo run
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
- `docker/proj/sh/*` - helper sh-scripts used in `echo-proj` docker container
- `Cargo.toml`, `Cargo.lock` - [cargo](https://doc.rust-lang.org/cargo/) files


