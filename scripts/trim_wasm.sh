#!/bin/sh
### USAGE: ./trim_wasm.sh INPUT_PATH OUTPUT_PATH
### DESCRIPTION: Trims the wasm file to remove the debug information
### and other unnecessary data. This can reduce the size of the wasm
### file by more than 90%.
###
### ARGS:
###   INPUT_PATH: The path to the input wasm file.
###   OUTPUT_PATH: The path to the output wasm file.
usage() { grep '^###' "$0"  | sed 's/^### //g; s/^###//g'; }

set -eu
if [ "$#" -ne 2 ]; then
  usage
  exit 1
fi

# see https://github.com/WebAssembly/binaryen
wasm-opt -Oz -o "$2.temp.wasm" "$1"

# see https://github.com/rustwasm/wasm-snip
_snip_cmd="wasm-snip -o \"$2\""
# _snip_cmd="$_snip_cmd --skip-producers-section"
# _snip_cmd="$_snip_cmd --snip-rust-panicking-code"
# _snip_cmd="$_snip_cmd --snip-rust-fmt-code"
_snip_cmd="$_snip_cmd \"$2.temp.wasm\""

eval "$_snip_cmd"
rm "$2.temp.wasm"

wc -c "$1"
wc -c "$2"
twiggy garbage "$2"

