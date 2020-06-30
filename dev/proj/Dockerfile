ARG BW_RUST_DOCKER_IMAGE
ARG BW_RUST_DOCKER_TAG
FROM ${BW_RUST_DOCKER_IMAGE}:${BW_RUST_DOCKER_TAG}

RUN apk add build-base
RUN apk add dumb-init 
RUN apk add bash
RUN apk add openssl-dev
# ARG BW_RUST_DOCKER_IMAGE
# ARG BW_RUST_DOCKER_TAG
# FROM {$BW_RUST_DOCKER_IMAGE}:${BW_RUST_DOCKER_TAG}
#
# ARG BW_DUMB_INIT_VERSION
# RUN true && \
#     apt-get update && \
#     apt-get install -y curl && \
#     # === dumb-init >>>
#     # https://github.com/Yelp/dumb-init
#     curl https://github.com/Yelp/dumb-init/releases/download/v${BW_DUMB_INIT_VERSION}/dumb-init_${BW_DUMB_INIT_VERSION}_amd64 -L -o /usr/local/bin/dumb-init && \
#     chmod +x /usr/local/bin/dumb-init && \
#     # <<< dumb-init ===
#     apt-get install -y libssl-dev pkg-config && \
# true
