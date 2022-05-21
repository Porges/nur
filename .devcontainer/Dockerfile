# See here for image contents: https://github.com/microsoft/vscode-dev-containers/tree/v0.233.0/containers/rust/.devcontainer/base.Dockerfile

# [Choice] Debian OS version (use bullseye on local arm64/Apple Silicon): buster, bullseye
ARG VARIANT="buster"
FROM mcr.microsoft.com/vscode/devcontainers/rust:0-${VARIANT}

RUN apt-get update && export DEBIAN_FRONTEND=noninteractive \
    && apt-get -y install --no-install-recommends \
    jq

USER vscode
RUN cargo install cargo-edit cargo-audit cargo-release git-cliff
