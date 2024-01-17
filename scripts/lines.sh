#!/usr/bin/env bash
lines() {
  local start=$1
  local end=$2
  ( if [ -z "${3:-}" ]; then cat -; else cat "$3"; fi ) |
  head "-$end" |
  tail "+$start"
}
if [ "${BASH_SOURCE[0]}" = "$0" ]; then line "$@"; fi
