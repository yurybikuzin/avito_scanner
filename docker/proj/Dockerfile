ARG BW_RUST_DOCKER_TAG
FROM rust:${BW_RUST_DOCKER_TAG}

ARG BW_DUMB_INIT_VERSION
RUN true && \
    apt-get update && \
    apt-get install -y curl && \
    # === dumb-init >>>
    # https://github.com/Yelp/dumb-init
    curl https://github.com/Yelp/dumb-init/releases/download/v${BW_DUMB_INIT_VERSION}/dumb-init_${BW_DUMB_INIT_VERSION}_amd64 -L -o /usr/local/bin/dumb-init && \
    chmod +x /usr/local/bin/dumb-init && \
    # <<< dumb-init ===
    apt-get install -y libssl-dev pkg-config && \
true
