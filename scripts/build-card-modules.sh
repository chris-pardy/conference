#!/usr/bin/env bash
# Builds the demo card modules in cards/ to wasm, into the server's seed
# data. The built files are committed, so building the server doesn't need
# the wasm32 target.
set -euo pipefail
cd "$(dirname "$0")/.."
cargo build --manifest-path cards/Cargo.toml --target wasm32-unknown-unknown --release
mkdir -p crates/server/demo
for m in std dice quiz event; do
  cp "cards/target/wasm32-unknown-unknown/release/card_$m.wasm" "crates/server/demo/$m.wasm"
done
ls -l crates/server/demo/*.wasm
