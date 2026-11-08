import pytest

import container_image_dist_ref as cidr


DIGEST = "sha256:" + "a" * 64


def test_parse_full_reference():
    ref = cidr.parse(f"docker.io/library/alpine:3.14@{DIGEST}")
    assert ref.name == "docker.io/library/alpine"
    assert str(ref.domain) == "docker.io"
    assert ref.domain.host.kind == "name"
    assert ref.domain.port is None
    assert ref.port is None
    assert str(ref.path) == "library/alpine"
    assert ref.path.parts == ["library", "alpine"]
    assert str(ref.tag) == "3.14"
    assert str(ref.digest) == DIGEST
    assert str(ref.digest.algorithm) == "sha256"
    assert ref.digest.algorithm.parts == ["sha256"]
    assert str(ref.digest.encoded) == "a" * 64
    assert ref.digest.compliance == "universal"
    assert ref.digest.is_compliant_with("oci")
    assert ref.digest.is_compliant_with("distribution")
    assert ref.is_canonical()
    assert str(ref) == f"docker.io/library/alpine:3.14@{DIGEST}"
    assert repr(ref).startswith("ImgRef(")
    assert str(ref) in repr(ref)


def test_parse_minimal_and_optional_parts():
    ref = cidr.parse("alpine")
    assert ref.name == "alpine"
    assert ref.domain is None
    assert ref.port is None
    assert str(ref.path) == "alpine"
    assert ref.tag is None
    assert ref.digest is None
    assert not ref.is_canonical()


def test_ipv6_and_port():
    ref = cidr.parse("[2001:db8::1]:5000/repo:tag")
    assert str(ref.domain) == "[2001:db8::1]:5000"
    assert ref.domain.host.kind == "ipv6"
    assert ref.domain.port == "5000"
    assert ref.port == "5000"
    assert str(ref.path) == "repo"


def test_canonical():
    canonical = cidr.parse_canonical(f"host.com/repo@{DIGEST}")
    assert canonical.name == "host.com/repo"
    assert str(canonical.domain) == "host.com"
    assert str(canonical.path) == "repo"
    assert canonical.tag is None
    assert str(canonical.digest) == DIGEST
    assert canonical.to_img_ref() == cidr.parse(f"host.com/repo@{DIGEST}")

    with pytest.raises(cidr.ParseError):
        cidr.parse_canonical("host.com/repo:tag")


def test_parse_errors_are_structured():
    with pytest.raises(cidr.ParseError) as excinfo:
        cidr.parse("[::]0")

    error = excinfo.value
    assert isinstance(error, ValueError)
    assert error.kind == "PortOrTagInvalidChar"
    assert error.index == 4
    assert error.kind in str(error)
    assert "4" in str(error)


def test_parse_invalid_tag_reports_offending_byte():
    with pytest.raises(cidr.ParseError) as excinfo:
        cidr.parse("name:ta g")

    assert excinfo.value.kind == "PortOrTagInvalidChar"
    assert excinfo.value.index == 7


def test_equality_and_hashing():
    assert cidr.parse("a/b") == cidr.parse("a/b")
    assert cidr.parse("a/b") != cidr.parse("a/c")
    assert hash(cidr.parse("a/b")) == hash(cidr.parse("a/b"))


def test_standalone_components():
    assert str(cidr.Domain("localhost:5000")) == "localhost:5000"
    assert str(cidr.Domain("localhost:5000").host) == "localhost"
    assert cidr.Domain("localhost:5000").port == "5000"
    assert str(cidr.Host("docker.io")) == "docker.io"
    assert cidr.Host("docker.io").kind == "name"
    assert str(cidr.Path("a/b")) == "a/b"
    assert cidr.Path("a/b").parts == ["a", "b"]
    assert str(cidr.Tag("v1.2.3")) == "v1.2.3"
    assert str(cidr.Algorithm("sha256")) == "sha256"
    assert str(cidr.Encoded("deadbeef")) == "deadbeef"
    assert str(cidr.Digest(DIGEST)) == DIGEST


def test_unknown_standard_is_rejected():
    with pytest.raises(ValueError):
        cidr.Digest(DIGEST).is_compliant_with("bogus")
    with pytest.raises(ValueError):
        cidr.Encoded("deadbeef", "bogus")


def test_version():
    assert cidr.__version__ == "0.3.0"
