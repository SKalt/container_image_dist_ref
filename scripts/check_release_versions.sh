#!/usr/bin/env bash
# Verify the release tag matches the versions in the package manifests.
#
# The tag (for example v0.3.0) is read from GITHUB_REF_NAME. Exits non-zero if
# any manifest version disagrees with the tag.
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$repo_root"

python3 - <<'PY'
import os
import pathlib
import tomllib

root = pathlib.Path(".")
pyproject = tomllib.loads((root / "bindings/python/pyproject.toml").read_text())
cargo = tomllib.loads((root / "Cargo.toml").read_text())
binding = tomllib.loads((root / "bindings/python/Cargo.toml").read_text())

versions = {
    "bindings/python/pyproject.toml": pyproject["project"]["version"],
    "Cargo.toml": cargo["package"]["version"],
    "bindings/python/Cargo.toml": binding["package"]["version"],
}

ref = os.environ["GITHUB_REF_NAME"]
expected = ref[1:] if ref.startswith("v") else ref
mismatched = {
    path: version for path, version in versions.items() if version != expected
}
if mismatched:
    raise SystemExit(f"tag {ref!r} does not match package versions: {mismatched}")
print(f"all package versions match {ref}")
PY
