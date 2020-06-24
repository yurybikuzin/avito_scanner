ARG BW_NODE_VERSION
FROM node:${BW_NODE_VERSION}

ARG BW_DUMB_INIT_VERSION
RUN true && \
    apt-get update && \
    apt-get install -y curl gnupg ca-certificates && \
    curl -s https://dl-ssl.google.com/linux/linux_signing_key.pub | apt-key add - && \
    sh -c 'echo "deb [arch=amd64] http://dl.google.com/linux/chrome/deb/ stable main" >> /etc/apt/sources.list.d/google.list' && \
    apt-get update && \
    apt-get install -y google-chrome-stable && \
    rm -rf /var/lib/apt/lists/* && \
    # === dumb-init >>>
    # https://github.com/Yelp/dumb-init
    curl https://github.com/Yelp/dumb-init/releases/download/v${BW_DUMB_INIT_VERSION}/dumb-init_${BW_DUMB_INIT_VERSION}_amd64 -L -o /usr/local/bin/dumb-init && \
    chmod +x /usr/local/bin/dumb-init && \
    # <<< dumb-init ===
true

RUN mkdir /proj

WORKDIR /proj

COPY ./src/package.json .
COPY ./src/yarn.lock .
COPY ./src/server.js .

RUN yarn

