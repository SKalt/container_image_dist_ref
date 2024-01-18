# container_image_dist_ref

A docker/OCI image reference parser.

[![Crates.io](https://img.shields.io/crates/v/container_image_dist_ref.svg)](https://crates.io/crates/container_image_dist_ref)
[![docs.rs](https://img.shields.io/docsrs/container_image_dist_ref)](https://docs.rs/container_image_dist_ref/latest/container_image_dist_ref/)

This library is extensively tested against the authoritative image reference implementation, https://github.com/distribution/reference.
`distribution/reference` uses the following [EBNF](https://www.w3.org/TR/xml11/#sec-notation) grammar:

<!-- {{{sh cat ./grammars/reference.ebnf }}}{{{out skip=2 -->

```ebnf
reference            ::= name (":" tag )? ("@" digest )?
name                 ::= (domain "/")? path
domain               ::= host (":" port-number)?
host                 ::= domain-name | IPv4address | "[" IPv6address "]" /* see https://www.rfc-editor.org/rfc/rfc3986#appendix-A */
domain-name          ::= domain-component ("." domain-component)*
domain-component     ::= ([a-zA-Z0-9]|[a-zA-Z0-9][a-zA-Z0-9-]*[a-zA-Z0-9])
port-number          ::= [0-9]+
path-component       ::= [a-z0-9]+ (separator [a-z0-9]+)*
path                 ::= path-component ("/" path-component)*
separator            ::= [_.] | "__" | "-"+

tag                  ::= [\w][\w.-]{0,127}

digest               ::= algorithm ":" encoded
algorithm            ::= algorithm-component (algorithm-separator algorithm-component)*
algorithm-separator  ::= [+._-]
algorithm-component  ::= [A-Za-z][A-Za-z0-9]*
encoded              ::= [a-fA-F0-9]{32,} /* At least 128 bit digest value */

identifier           ::= [a-f0-9]{64}
```

<!-- }}} skip=2 -->

(This is translated from [https://github.com/distribution/reference/blob/main/reference.go](https://github.com/distribution/reference/blob/main/reference.go#L4-L26))

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
