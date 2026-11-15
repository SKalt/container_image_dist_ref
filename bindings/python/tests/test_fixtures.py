"""Corpus tests using the distribution/reference fixtures.

The fixtures in ``tests/fixtures/references`` are copied from
https://github.com/distribution/reference (Apache-2.0). ``outputs.tsv`` records
the expected decomposition of every valid reference.
"""

import csv
from pathlib import Path

import pytest

import container_image_dist_ref as cidr


FIXTURES = Path(__file__).resolve().parents[3] / "tests" / "fixtures" / "references"


def _lines(path):
    return [line for line in path.read_text().splitlines() if line]


def _known_divergences():
    return set(_lines(FIXTURES / "xfail.txt"))


def _expected_rows():
    with (FIXTURES / "outputs.tsv").open(newline="") as file:
        rows = list(csv.DictReader(file, delimiter="\t"))
    divergences = _known_divergences()
    return [row for row in rows if not row["err"] and row["input"] not in divergences]


VALID_ROWS = _expected_rows()
INVALID_INPUTS = _lines(FIXTURES / "invalid" / "inputs.txt")


def test_fixture_files_are_present():
    assert (FIXTURES / "outputs.tsv").is_file()
    assert (FIXTURES / "valid" / "inputs.txt").is_file()
    assert (FIXTURES / "invalid" / "inputs.txt").is_file()
    assert VALID_ROWS
    assert INVALID_INPUTS


@pytest.mark.parametrize(
    "row", VALID_ROWS, ids=[row["input"][:60] for row in VALID_ROWS]
)
def test_valid_references_match_expected_decomposition(row):
    ref = cidr.parse(row["input"])

    assert ref.name == row["name"]
    assert (str(ref.domain) if ref.domain else "") == row["domain"]
    assert str(ref.path) == row["path"]
    assert (str(ref.tag) if ref.tag else "") == row["tag"]

    if row["digest_algo"]:
        assert ref.digest is not None
        assert str(ref.digest.algorithm) == row["digest_algo"]
        assert str(ref.digest.encoded) == row["digest_encoded"]
    else:
        assert ref.digest is None


@pytest.mark.parametrize(
    "src", INVALID_INPUTS, ids=[src[:60] for src in INVALID_INPUTS]
)
def test_invalid_references_are_rejected(src):
    with pytest.raises(cidr.ParseError) as excinfo:
        cidr.parse(src)

    error = excinfo.value
    assert isinstance(error.kind, str)
    assert 0 <= error.index <= len(src.encode())
