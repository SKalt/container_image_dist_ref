# Architecture

This document records this project's design principles.
Inspired by https://matklad.github.io/2021/02/06/ARCHITECTURE.md.html

## Fun optimizations

The speed and memory footprint of image reference parsing is highly unlikely to ever matter to a program handling gigabytes of images.
However, optimizing the parsing is fun, and fun is encouraged in this repository.

### Parse ascii bytes

This library takes an input ascii string (a slice of bytes) and parses the lengths of each of the sections of an image reference.
Using ascii only avoids allocating unicode `char`s which each weigh 4 bytes.

<!-- I learned this optimization from the `regex` crate! -->

### Avoid backtracking

Re-parsing bytes costs time and memory.
Peeking one byte ahead is ok.
Re-parsing sections on error to find an invalid character is also ok as long as the benchmarks don't regress.

### Keep only one copy of a string slice

`&str`s are expensive: they cost 2 `usize`s.
Prefer holding one `&str` and many short lengths in-memory, then splitting new `&str`s using the lengths on-demand.

### Store short lengths

Use the smallest unsigned integer size that can represent the length of a section of an image reference.
Since most sections of an image reference are under 255 ascii characters long, most lengths can be represented using a `u8`.
The encoded section of the digest is technically unbounded, but practically can be measured with a `u16`.

### All lengths are implicitly optional

Since all lengths can be 0, treat 0 as the `None` value rather than using extra space for an `Option<Length>`.
Temporarily converting a length to an `Option<length>` is ok, since it's roughly equivalent to using a temporary bool while checking `len == 0`.

<!-- I'm not 100% on the Option<L>/bool equivalence, but I do know it's cheap -->

### debug mode

Record invariants using `debug_assert!(..)` instead of `assert!(..)` to avoid extra computation in release mode.
Put extra debugging variables behind [`#[cfg(debug_assertions)]` conditional-compilation](https://doc.rust-lang.org/reference/conditional-compilation.html#debug_assertions) macros.

### 0 dependencies

To keep the library size small and keep ownership of all of the relevant logic.

I chose not to use the excellent [`regex` crate](https://github.com/rust-lang/regex) since:

1. writing the parsers as pure functions avoids issues of [cross-thread resource contention](https://docs.rs/regex/latest/regex/#sharing-a-regex-across-threads-can-result-in-contention).
1. I _think_ `regex` relies on pointer-sized offsets for capture groups, which cancels out the short-length optimizations. A scan through the `regex` and `regex-automata` docs and issues didn't reveal a way to use `u8`s .
   If you know a way to get `regex` to use custom offset sizes, please let me know in [this repo's issues](https://github.com/SKalt/container_image_dist_ref/issues)!
