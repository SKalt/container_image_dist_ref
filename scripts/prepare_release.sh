#!/usr/bin/env bash
make test
make link-check
cargo publish --dry-run


# [ ] update version in ./Cargo.toml
# [ ] tag commit with the NEW version
# [ ] push tag to github
# [ ] upload the new crate to crates.io

