#!/usr/bin/env bash
# shellcheck disable=SC2296
if [[ "${BASH_SOURCE[0]}" = */* ]]; then this_dir="${BASH_SOURCE[0]%/*}"; # bash
elif [ -n "${(%):-%N}" ]; then this_dir="${(%):-%N}";                     # zsh
else this_dir=.;
fi
# shellcheck source=./lines.sh
. "$this_dir/lines.sh"
if [ ! -f /tmp/oci_name_grammar ]; then
  curl -s "https://raw.githubusercontent.com/opencontainers/distribution-spec/main/spec.md" > /tmp/oci_name_grammar
fi
escaped() {
  printf "%s" "$1" |
  sed '
    s/\\/\\\\/g
    s/\[/\\[/g
    s/\]/\\]/g
    s/[*]/[*]/g
    s/[+]/[+]/g
    s/[(]/[(]/g
    s/[)]/[)]/g
    s#/#\/#g
  ';
}
lower_alnum='[a-z0-9]+'
separator='(\.|_|__|-+)'
translated_separator='[_.] | "__" | "-"+'
path_component="$lower_alnum(separator $lower_alnum)*"
{
  printf "path                ::= "
  lines 156 156 /tmp/oci_name_grammar |
    sed 's/`//g' |
    sed "s/$(escaped "$lower_alnum")/$lower_alnum/g" |
    sed "s/$(escaped "$separator")/separator /g" |
    sed 's#\\/#"/" #g' |
    sed "s/$(escaped "$path_component")/path-component/g" |
    cat -;
  echo "path-component      ::= $path_component"
  echo "separator           ::= $translated_separator"
} | sed -E 's/([^\s])[(]/\1 (/g;'  # touch up space around ()s
