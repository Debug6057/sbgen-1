#!/usr/bin/env bash
# builds the wasm into web/pkg, run from the dev shell (nix develop)
set -euo pipefail
cd "$(dirname "$0")/.."
# the shell's RUSTFLAGS are for native builds (mold)
RUSTFLAGS="" cargo rustc --lib --crate-type cdylib --target wasm32-unknown-unknown --release
wasm-bindgen --target web --out-dir web/pkg --out-name sbgen target/wasm32-unknown-unknown/release/sbgen.wasm
wasm-opt -Oz -o web/pkg/sbgen_bg.wasm web/pkg/sbgen_bg.wasm
test -s web/pkg/sbgen_bg.wasm
