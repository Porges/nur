ARG VARIANT="bullseye"
FROM mcr.microsoft.com/vscode/devcontainers/rust:0-${VARIANT}

RUN apt-get update && export DEBIAN_FRONTEND=noninteractive \
    && apt-get -y install --no-install-recommends \
    jq

USER vscode
RUN rustup target add x86_64-unknown-linux-musl && rustup default stable
RUN cargo install cargo-edit cargo-deny cargo-workspaces git-cliff
