#!/usr/bin/env bash
# Build the X_ITE_R page module: crates/x3d-web for wasm32-unknown-unknown, then wasm-bindgen into
# crates/x3d-web/web/pkg (gitignored). The page imports it through crates/x3d-web/web/x_ite_r.mjs.
#
#   tools/build-web.sh [--debug]
#
# Needs the wasm32-unknown-unknown target (rustup target add wasm32-unknown-unknown) and wasm-bindgen-cli
# of the exact wasm-bindgen version in Cargo.lock (cargo install wasm-bindgen-cli --version <v> --locked).
# WASM_BINDGEN overrides the wasm-bindgen executable.
set -euo pipefail

cd "$(dirname "$0")/.."

profile=release
flags=(--release)

if [[ "${1:-}" == "--debug" ]]; then
  profile=debug
  flags=()
fi

want=$(awk '/^name = "wasm-bindgen"$/ { getline; gsub(/version = |"/, ""); print; exit }' Cargo.lock)
bindgen=${WASM_BINDGEN:-wasm-bindgen}
have=$("$bindgen" --version | awk '{ print $2 }')

if [[ "$have" != "$want" ]]; then
  echo "build-web: $bindgen is $have; Cargo.lock has wasm-bindgen $want" >&2
  exit 1
fi

cargo build "${flags[@]}" --target wasm32-unknown-unknown -p x3d-web

target_dir=$(cargo metadata --format-version 1 --no-deps | python3 -c 'import json, sys; print(json.load(sys.stdin)["target_directory"])')

"$bindgen" --target web --no-typescript --out-dir crates/x3d-web/web/pkg \
  "$target_dir/wasm32-unknown-unknown/$profile/x3d_web.wasm"

ls -l crates/x3d-web/web/pkg
