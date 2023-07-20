#!/bin/sh

set -eux

rustup default stable
rustup component add llvm-tools-preview clippy rustfmt
rustup target add x86_64-unknown-linux-musl

curl -L --proto '=https' --tlsv1.2 -sSf https://raw.githubusercontent.com/cargo-bins/cargo-binstall/main/install-from-binstall-release.sh | bash
cargo binstall --no-confirm cargo-deny cargo-workspaces git-cliff cargo-nextest cargo-llvm-cov cargo-outdated cargo-insta
