#!/bin/bash
set -e

if [ -d "/usr/matchmaking-state" ]; then
  if ! grep -q '\[patch.crates-io\]' Cargo.toml; then
    echo "[dev] Applying local library patches to Cargo.toml..."
    cat >> Cargo.toml << 'TOMLPATCH'

[patch.crates-io]
gn-matchmaking-state      = { path = "/usr/matchmaking-state" }
gn-matchmaking-state-types = { path = "/usr/matchmaking-state-types" }
gn-communicator           = { path = "/usr/communicator" }
gn-ranking-client-rs      = { path = "/usr/gn-ranking-client-rs" }
gn-redisadapter-derive    = { path = "/usr/matchmaking-state/redisadapter-derive" }
TOMLPATCH
  fi
  cargo build
fi

exec ./target/debug/games-agent
