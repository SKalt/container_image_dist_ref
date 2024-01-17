# container_image_dist_ref

A docker/OCI image reference parser.

[![Crates.io](https://img.shields.io/crates/v/container_image_dist_ref.svg)](https://crates.io/crates/container_image_dist_ref)

This library is extensively tested against the authoritative image reference implementation, https://github.com/distribution/reference.

## Motivation

<!-- TODO: rewrite -->

I wanted to use `distribution/reference` in a rust project, but didn't want to deal with FFI into `go`.

## Goals

1. fidelity to the `distribution/reference`'s parser
1. fun optimizations!
<!-- 1. The eventual ability to re-use the parser in other languages -->

More about these goals and design choices in [`./ARCHITECTURE.md`](./ARCHITECTURE.md).

## Benchmarks

Based on some naive benchmarking, this library achieves at least a 10x speedup compared to distribution/reference.

<details open><summary>Running the benchmarks</summary>

```sh
#!/bin/bash
cargo bench # rust
( # go
  cd internal/reference_oracle &&
  go test -bench=.
)
```

</details>

<details><summary>Benchmarks on my machine</summary>

distribution/reference:

```
goos: linux
goarch: amd64
pkg: github.com/skalt/container_image_dist_ref/internal/reference_oracle
cpu: Intel(R) Core(TM) i7-4770 CPU @ 3.40GHz
BenchmarkOracleEntireTestSuite-8            9218            148438 ns/op
```

This crate:

```
entire_test_suite       time:   [5.0737 µs 5.1349 µs 5.2047 µs]
```

```
speedup = (148438 ns) / ((5.1349 µs) * (1000 ns / µs)) = 28.908
```

</details>
