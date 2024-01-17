#!/usr/bin/env bash
# CWD should be the root of the repository
head -2 ./grammars/digest.diff;
grep -E '^.algorithm' ./grammars/digest.diff;
grep -E '^.component' ./grammars/digest.diff;
grep -E '^.separator' ./grammars/digest.diff;
