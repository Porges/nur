ARG VARIANT="bullseye"
FROM mcr.microsoft.com/vscode/devcontainers/rust:${VARIANT}

RUN apt-get update && export DEBIAN_FRONTEND=noninteractive \
    && apt-get -y install --no-install-recommends \
    jq

USER vscode

COPY install.sh .
RUN ./install.sh

USER root
RUN rm install.sh
