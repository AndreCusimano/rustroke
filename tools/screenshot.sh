#!/bin/sh
# Runs an example, captures ONLY its window (never the whole screen), and quits.
# macOS only; needs Screen Recording permission for the terminal.
#
# Usage: tools/screenshot.sh <example> [output.png] [seconds]
set -eu
example=$1
out=${2:-target/screenshots/$example.png}
wait=${3:-3}
root=$(cd "$(dirname "$0")/.." && pwd)
helper=$root/target/tools/window_id

mkdir -p "$root/target/tools" "$(dirname "$out")"
if [ ! -x "$helper" ] || [ "$root/tools/window_id.swift" -nt "$helper" ]; then
    swiftc -O "$root/tools/window_id.swift" -o "$helper"
fi
cargo build -q -p rustroke --example "$example"

"$root/target/debug/examples/$example" >/dev/null 2>&1 &
pid=$!
trap 'kill $pid 2>/dev/null || true' EXIT
sleep "$wait"
wid=$("$helper" "$pid")
if [ -z "$wid" ]; then
    echo "no window found for $example (pid $pid)" >&2
    exit 1
fi
screencapture -x -o -l "$wid" "$out"
echo "$out"
