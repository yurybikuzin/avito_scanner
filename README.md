= Avito scanner, written in Rust

== File structure

- README.md - this is it
- .gitignore - see https://git-scm.com/docs/gitignore
- .gitlab-ci.yml - required for proper work of `docker/refresh.sh`
- `docker-compose.yml` - required for `docker-compose up -d`
- `.env` - required for `docker-compose up -d`
- docker/refresh.sh - tool for rebuilding docker container from docker/\*, in particular `avito-proj` 
- docker/proj/Dockerfile - Dockerfile for service `proj`, mentioned in `docker-compose.yml`
- docker/proj/sh/ - folder for helper sh-scripts used in `avito-proj` docker container

== Development

=== Prerequisites

- git
- docker 
- docker-compose

=== Prepare workplace

- clone repo: 

```
git clone 
```

- up dev container: 

```
docker-compose up -d && docker logs avito-proj -f
```

==== Available commands

===== cargo

```
docker exec -it avito-proj cargo
docker exec -it -e RUST_LOG=info avito-proj cargo test -p diaps
docker exec -it avito-proj cargo run
```

=== Don't forget stop docker containers after work is over

```
docker-compose down
```

https://vimeo.com/user58195081/review/394860047/b827eafd0d
23:43-24:18

