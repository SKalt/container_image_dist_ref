#!/usr/bin/env bash
# shellcheck disable=SC2296
if [[ "${BASH_SOURCE[0]}" = */* ]]; then this_dir="${BASH_SOURCE[0]%/*}"; # bash
elif [ -n "${(%):-%N}" ]; then this_dir="${(%):-%N}";                     # zsh
else this_dir=.;
fi
# shellcheck source=./lines.sh
. "$this_dir/lines.sh"

if [ ! -f /tmp/oci_digest_grammar ]; then
  curl -s "https://raw.githubusercontent.com/opencontainers/image-spec/v1.0.2/descriptor.md" > /tmp/oci_digest_grammar
fi
lines 72 76 /tmp/oci_digest_grammar |
  sed 's/   ::=/  ::=/g' |
  sed 's///g' |
  cat -
  # sed 's/algorithm/digest-algorithm/g' |
