#!/bin/bash
set -e

if [ -d "/usr/schnapsen-rs" ]; then
  if ! grep -q '\[patch.crates-io\]' Cargo.toml; then
    echo "[dev] Applying local library patches to Cargo.toml..."
    cat >> Cargo.toml << 'TOMLPATCH'

[patch.crates-io]
schnapsen-rs    = { path = "/usr/schnapsen-rs" }
gn-communicator = { path = "/usr/communicator" }
TOMLPATCH
  fi
  cargo build
fi

exec ./target/debug/schnapsen-duo-server
