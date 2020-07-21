ARG BW_PROD_PROXY_DOCKER_IMAGE
ARG BW_PROD_PROXY_DOCKER_TAG
FROM ${BW_PROD_PROXY_DOCKER_IMAGE}:${BW_PROD_PROXY_DOCKER_TAG}

# RUN apk add dumb-init bash
RUN apt-get update && apt-get install -y dumb-init bash
RUN apt-get install -y libssl1.1

# https://www.fpcomplete.com/blog/2018/07/deploying-rust-with-docker-and-kubernetes/
ENV DEBIAN_FRONTEND=noninteractive
RUN apt-get update && apt-get -y install ca-certificates libssl-dev && rm -rf /var/lib/apt/lists/*

RUN mkdir /proj
WORKDIR /proj
COPY ./copy/scan .
ENV PATH="/proj/:${PATH}"
#
# RUN apk add openssl-dev
# RUN apk add build-base
# RUN apk add util-linux
# # RUN apk add dumb-init 
# # RUN apk add bash
#
ENTRYPOINT [ "dumb-init", "--",  "bash" ]
# ENTRYPOINT [ "dumb-init", "--",  "bash" ]


# ARG BW_PROD_PROJ_DOCKER_IMAGE
# ARG BW_PROD_PROJ_DOCKER_TAG
# FROM ${BW_PROD_PROJ_DOCKER_IMAGE}:${BW_PROD_PROJ_DOCKER_TAG}
#
# # RUN apk add dumb-init bash
# RUN apt-get update && apt-get install -y dumb-init bash
#
# RUN mkdir /proj
# WORKDIR /proj
# COPY ./copy/scan .
# ENV PATH="/proj/:${PATH}"
# #
# # RUN apk add openssl-dev
# # RUN apk add build-base
# # RUN apk add util-linux
# # # RUN apk add dumb-init 
# # # RUN apk add bash
# #
# ENTRYPOINT [ "dumb-init", "--",  "bash" ]
