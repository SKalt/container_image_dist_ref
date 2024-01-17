#!/usr/bin/env bash
# CWD should be the root of the repository
head -2 ./grammars/digest.diff;
grep -E '^.encoded' ./grammars/digest.diff | sed 's/            //g'
