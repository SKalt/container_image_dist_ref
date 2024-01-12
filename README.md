# container_image_dist_ref

A library of types represent docker/OCI image references.

A rust port of https://github.com/distribution/reference

## Motivation

(1) fidelity to the original flagship image reference implementation and (2) fun optimizations.

## Benchmarks

Compared to distribution/reference, this achieves a 25x speedup:

```
goos: linux
goarch: amd64
pkg: github.com/skalt/container_image_dist_ref/scripts/bench_oracle
cpu: Intel(R) Core(TM) i7-4770 CPU @ 3.40GHz
BenchmarkOracle-8           8851            123423 ns/op
PASS
ok      github.com/skalt/container_image_dist_ref/scripts/bench_oracle  1.111s
```

```
entire_test_suite       time:   [4.9178 µs 4.9369 µs 4.9592 µs]
Found 7 outliers among 100 measurements (7.00%)
  3 (3.00%) high mild
  4 (4.00%) high severe
```

<!--
A note on regex: I chose not to use the excellent github.com/rust-lang/regex since:
  1. this kind of parsing isn't really regex-shaped: we're parsing strings from start to finish, not looking for needles in haystacks.
  2. writing the parser as a pure function avoids all issues of cross-thread resource contention (https://docs.rs/regex/latest/regex/#sharing-a-regex-across-threads-can-result-in-contention) and lets me use the smallest unsigned int size possible for each section of the reference string.
-->
