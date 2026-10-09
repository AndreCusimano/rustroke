#!/bin/sh
# Builds an example for the browser into target/web/<example>/ (open
# index.html through any static web server, e.g.
# `python3 -m http.server -d target/web/<example>`).
#
# Needs: rustup target add wasm32-unknown-unknown
#        cargo install wasm-bindgen-cli --version <the wasm-bindgen version in Cargo.lock>
#
# Usage: tools/web.sh <example> [extra cargo args, e.g. --features markdown]
set -eu
example=$1
shift
root=$(cd "$(dirname "$0")/.." && pwd)
out=$root/target/web/$example
cargo build --release -p rustroke --example "$example" --target wasm32-unknown-unknown "$@"
wasm-bindgen --target web --no-typescript --out-dir "$out/pkg" \
    "$root/target/wasm32-unknown-unknown/release/examples/$example.wasm"
sed "s/EXAMPLE/$example/g" "$root/tools/web.html" > "$out/index.html"
echo "$out/index.html"
