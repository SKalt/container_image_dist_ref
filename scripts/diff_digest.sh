#!/usr/bin/env bash

# grammar="$(./scripts/lines.sh 14 18 ./grammars/reference.ebnf)"
use_rule() {  grep -E "^$2 " "$1"; }

fake_diff_line() {
  local rule="$1"
  local oci ref
  ref="$(use_rule ./grammars/reference.ebnf "$rule")"
  oci="$(use_rule ./grammars/oci_digest.ebnf "$rule")"
  if [ "$oci" = "$ref" ]; then
    printf " %s\n" "$oci";
  else
   printf "-%s\n" "$ref";
   printf "+%s\n" "$oci";
  fi
}

fake_diff() {
  echo "--- distribution/reference"
  echo "+++ opencontainers/image-spec"
  {
    fake_diff_line "digest"
    fake_diff_line "algorithm"
    fake_diff_line "algorithm-component"
    fake_diff_line "algorithm-separator"
    fake_diff_line "encoded"
  } |
    sed '
      s/algorithm-component/component/g
      s/algorithm-separator/separator/g
    ' |
    sed 's/::=/~/g' |
    column -s~ -t -o "::="
}

fake_diff
