#!/usr/bin/env bash

# shellcheck disable=SC2296
if [[ "${BASH_SOURCE[0]}" = */* ]]; then this_dir="${BASH_SOURCE[0]%/*}"; # bash
elif [ -n "${(%):-%N}" ]; then this_dir="${(%):-%N}";                     # zsh
else this_dir=.;
fi
# shellcheck source=./lines.sh
. "$this_dir/lines.sh"

if [ ! -f /tmp/reference.go ]; then
  curl -s "https://raw.githubusercontent.com/distribution/reference/main/reference.go" > /tmp/reference.go
fi

cat /tmp/reference.go |
  lines 6 26 |
  sed '
    # remove the golang-specific decorations
    s#^//##g; s#^\t##g;
    s/[(]or "remote-name"[)]/                  /g;
    s/remote-name/path/g; # rename "remote-name" -> "path"
  ' |
  sed "s/'/\"/g" | # make character quoting consistent
  sed 's/:=/::=/g' | # use ::= for production rules
  awk ' # replace  [...]  with (...) for grouping
    {
      if (match($0, /:= \//)) {
        print $0
      } else {
        gsub( / \[ ?/   , " ("    , $0)
        gsub( / ?\]\*/ , ")* "  , $0)
        gsub( /\] /    , ")? "  , $0)
        gsub( /\]$/    , ")?"   , $0)
        print $0
      }
    }
  ' |
  sed 's/\\\[/"["/g; s/\\\]/"]"/g' | # \[ ... \] -> "[" ... "]"
  sed 's# /# #g; s#/ # #g; s#/$##g' | # remove regex borders
  sed 's/\t/ /g' | # remove remaining tabs
  awk '
    {
      if (match ($0, /;/)) {
        gsub(/;/, "/*", $0); gsub(/$/, " */", $0);
        print $0
      } else {
        print $0
      }
    }
  ' |
  sed 's/|__|/ | "__" | /g' |
  sed 's/\[-\]\*/"-"+/g' | # replace [-]* with "-"+, which makes the rule non-optional to preserve the author's intent.
  sed 's/rfc3986 appendix-A/see https:\/\/www.rfc-editor.org\/rfc\/rfc3986#appendix-A/g' |
  grep -ve '^alpha-numeric' |
  sed 's/alpha-numeric/[a-z0-9]+/g' |
  sed -E '
    s/^digest-algorithm-(\w+)(\s+)::=/algorithm-\1\2       ::=/g;
    s/^digest-algorithm(\s+)::=/algorithm\1       ::=/g;
    s/digest-algorithm/algorithm/g;
    s/digest-hex(\s+)::=/encoded\1   ::=/g;
    s/digest-hex/encoded/g;
  ' |
  sed 's/            ::=/ ::=/g' |
  sed -E 's/\s+$//g' |
  sed ' # for visual consistency with the OCI grammar
    s/\[[+][.][-][_]\]/[+._-]/g;
    s/\[0-9a-fA-F\]/[a-fA-F0-9]/g;
  ' |
  cat -
