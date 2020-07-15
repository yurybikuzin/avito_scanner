ARG BW_PROD_PROXY_DOCKER_IMAGE
ARG BW_PROD_PROXY_DOCKER_TAG
FROM ${BW_PROD_PROXY_DOCKER_IMAGE}:${BW_PROD_PROXY_DOCKER_TAG}

RUN apk add dumb-init bash
# RUN apt-get update && apt-get install -y dumb-init bash

RUN mkdir /proj
WORKDIR /proj
COPY ./copy/proxy .
ENV PATH="/proj/:${PATH}"
#
# RUN apk add openssl-dev
# RUN apk add build-base
# RUN apk add util-linux
# # RUN apk add dumb-init 
# # RUN apk add bash
#
ENTRYPOINT [ "dumb-init", "--",  "proxy" ]
