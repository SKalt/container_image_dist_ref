import container_image_dist_ref as dist
import pytest

DIGEST = "sha256:" + "a" * 64


def test_parse_full_reference():
    ref = dist.parse(f"docker.io/library/alpine:3.14@{DIGEST}")
    assert ref.name == "docker.io/library/alpine"
    assert ref.domain is not None
    assert str(ref.domain) == "docker.io"
    assert ref.domain.host.kind == "name"
    assert ref.domain.port is None
    assert ref.port is None
    assert str(ref.path) == "library/alpine"
    assert ref.path.parts == ["library", "alpine"]
    assert ref.tag is not None
    assert str(ref.tag) == "3.14"
    assert ref.digest is not None
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
    ref = dist.parse("alpine")
    assert ref.name == "alpine"
    assert ref.domain is None
    assert ref.port is None
    assert str(ref.path) == "alpine"
    assert ref.tag is None
    assert ref.digest is None
    assert not ref.is_canonical()


def test_ipv6_and_port():
    ref = dist.parse("[2001:db8::1]:5000/repo:tag")
    assert ref.domain is not None
    assert str(ref.domain) == "[2001:db8::1]:5000"
    assert ref.domain.host.kind == "ipv6"
    assert ref.domain.port == "5000"
    assert ref.port == "5000"
    assert str(ref.path) == "repo"


def test_canonical():
    canonical = dist.parse_canonical(f"host.com/repo@{DIGEST}")
    assert canonical.name == "host.com/repo"
    assert str(canonical.domain) == "host.com"
    assert str(canonical.path) == "repo"
    assert canonical.tag is None
    assert str(canonical.digest) == DIGEST
    assert canonical.to_img_ref() == dist.parse(f"host.com/repo@{DIGEST}")

    with pytest.raises(dist.ParseError):
        dist.parse_canonical("host.com/repo:tag")


def test_parse_errors_are_structured():
    with pytest.raises(dist.ParseError) as excinfo:
        dist.parse("[::]0")

    error = excinfo.value
    assert isinstance(error, ValueError)
    assert error.kind == "PortOrTagInvalidChar"
    assert error.index == 4
    assert error.kind in str(error)
    assert "4" in str(error)


def test_parse_invalid_tag_reports_offending_byte():
    with pytest.raises(dist.ParseError) as excinfo:
        dist.parse("name:ta g")

    assert excinfo.value.kind == "PortOrTagInvalidChar"
    assert excinfo.value.index == 7


def test_equality_and_hashing():
    assert dist.parse("a/b") == dist.parse("a/b")
    assert dist.parse("a/b") != dist.parse("a/c")
    assert hash(dist.parse("a/b")) == hash(dist.parse("a/b"))


def test_standalone_components():
    assert str(dist.Domain("localhost:5000")) == "localhost:5000"
    assert str(dist.Domain("localhost:5000").host) == "localhost"
    assert dist.Domain("localhost:5000").port == "5000"
    assert str(dist.Host("docker.io")) == "docker.io"
    assert dist.Host("docker.io").kind == "name"
    assert str(dist.Path("a/b")) == "a/b"
    assert dist.Path("a/b").parts == ["a", "b"]
    assert str(dist.Tag("v1.2.3")) == "v1.2.3"
    assert str(dist.Algorithm("sha256")) == "sha256"
    assert str(dist.Encoded("deadbeef")) == "deadbeef"
    assert str(dist.Digest(DIGEST)) == DIGEST


def test_unknown_standard_is_rejected():
    with pytest.raises(ValueError):
        dist.Digest(DIGEST).is_compliant_with("bogus")
    with pytest.raises(ValueError):
        dist.Encoded("deadbeef", "bogus")


def test_version():
    assert dist.__version__ == "0.4.0"


@pytest.mark.parametrize(
    "src",
    [
        "alpine",
        "docker.io/library/alpine:3.14",
        "localhost:5000/a/b",
        "[2001:db8::1]:5000/repo:tag@" + DIGEST,
    ],
)
def test_parse_is_equivalent_to_imgref_constructor(src):
    assert dist.parse(src) == dist.ImgRef(src)


def test_parse_component_helpers_match_constructors():
    canonical_src = "host.com/repo:tag@" + DIGEST
    pairs = [
        (dist.parse_canonical(canonical_src), dist.CanonicalImgRef(canonical_src)),
        (dist.parse_digest(DIGEST), dist.Digest(DIGEST)),
        (dist.parse_algorithm("sha256+extra"), dist.Algorithm("sha256+extra")),
        (dist.parse_encoded("deadbeef"), dist.Encoded("deadbeef")),
        (dist.parse_domain("localhost:5000"), dist.Domain("localhost:5000")),
        (dist.parse_host("[::1]"), dist.Host("[::1]")),
        (dist.parse_path("library/alpine"), dist.Path("library/alpine")),
        (dist.parse_tag("v1.2.3"), dist.Tag("v1.2.3")),
    ]
    for parsed, constructed in pairs:
        assert parsed == constructed
        assert str(parsed) == str(constructed)


def test_component_parsers_stop_at_delimiters():
    assert str(dist.parse_path("repo:tag@" + DIGEST)) == "repo"
    assert str(dist.parse_path("repo@" + DIGEST)) == "repo"
    assert str(dist.parse_tag("tag@" + DIGEST)) == "tag"
    assert str(dist.Tag("tag@" + DIGEST)) == "tag"


def test_canonical_with_tag_and_port():
    src = "[2001:db8::1]:5000/repo:tag@" + DIGEST
    canonical = dist.parse_canonical(src)
    assert canonical.domain.port == "5000"
    assert canonical.port == "5000"
    assert str(canonical.tag) == "tag"
    assert str(canonical.digest) == DIGEST
    assert str(canonical) == src


def test_canonical_requires_domain_and_digest():
    for src in ["host.com/repo:tag", "repo@" + DIGEST, "host.com/repo"]:
        with pytest.raises(dist.ParseError):
            dist.parse_canonical(src)

    with pytest.raises(dist.ParseError):
        dist.parse("repo:tag").to_canonical()


@pytest.mark.parametrize(
    ("src", "expected"),
    [
        ("host.com/repo@" + DIGEST, True),
        ("host.com/repo:tag@" + DIGEST, True),
        ("localhost:5000/repo@" + DIGEST, True),
        ("host.com/repo:tag", False),
        ("repo@" + DIGEST, False),
        ("repo:tag", False),
    ],
)
def test_is_canonical(src, expected):
    assert dist.parse(src).is_canonical() is expected


def test_digest_compliance_variants():
    universal = dist.parse_digest(DIGEST)
    assert universal.compliance == "universal"
    assert universal.is_compliant_with("oci")
    assert universal.is_compliant_with("distribution")
    assert universal.algorithm.compliance == "universal"

    oci_only = dist.parse_digest("1sha:abcd")
    assert oci_only.compliance == "oci"
    assert oci_only.is_compliant_with("oci")
    assert not oci_only.is_compliant_with("distribution")
    assert oci_only.algorithm.compliance == "oci"

    distribution_only = dist.parse_digest("SHA256:" + "a" * 64)
    assert distribution_only.compliance == "distribution"
    assert not distribution_only.is_compliant_with("oci")
    assert distribution_only.is_compliant_with("distribution")
    assert distribution_only.algorithm.compliance == "distribution"


def test_algorithm_parts_and_compliance_with_separators():
    # Regression test: separators used to be treated as unreachable by
    # Algorithm.compliance().
    algorithm = dist.parse_algorithm("sha256+extra.more-part_pieces")
    assert algorithm.parts == ["sha256", "extra", "more", "part", "pieces"]
    assert algorithm.compliance == "universal"
    assert str(algorithm) == "sha256+extra.more-part_pieces"

    assert dist.Algorithm("sha256+1extra").compliance == "oci"
    assert dist.Algorithm("UPPER").compliance == "distribution"
    assert dist.Algorithm("1lower").compliance == "oci"


def test_encoded_respects_standard_character_sets():
    assert str(dist.parse_encoded("abcXYZ")) == "abcXYZ"
    assert str(dist.parse_encoded("abcXYZ", "oci")) == "abcXYZ"

    with pytest.raises(dist.ParseError) as excinfo:
        dist.parse_encoded("abcXYZ", "distribution")
    assert excinfo.value.kind == "EncodedNonLowerHex"
    assert excinfo.value.index == 3

    with pytest.raises(dist.ParseError) as excinfo:
        dist.parse_encoded("")
    assert excinfo.value.kind == "EncodedMissing"
    assert excinfo.value.index == 0


@pytest.mark.parametrize(
    ("src", "kind"),
    [
        ("docker.io", "name"),
        ("192.168.1.1", "name"),
        ("localhost", "name"),
        ("[::1]", "ipv6"),
    ],
)
def test_host_kinds(src, kind):
    host = dist.parse_host(src)
    assert str(host) == src
    assert host.kind == kind


@pytest.mark.parametrize(
    ("parser", "src", "kind", "index"),
    [
        (dist.parse_domain, "test.com:", "PortOrTagMissing", 9),
        (dist.parse_domain, "test.com:tag", "PortInvalidChar", 9),
        (dist.parse_domain, "test.com/path", "HostInvalidChar", 8),
        (dist.parse_domain, "", "HostMissing", 0),
        (dist.parse_host, "-bad.com", "HostInvalidChar", 0),
        (dist.parse_host, "a_b.com", "HostInvalidChar", 1),
        (dist.parse_host, "[fe80::1%eth0]", "Ipv6InvalidChar", 8),
        (dist.parse_path, "", "PathMissing", 0),
        (dist.parse_path, "a//b", "PathComponentInvalidEnd", 2),
        (dist.parse_path, "A/b", "PathInvalidChar", 0),
        (dist.parse_tag, "", "TagMissing", 0),
        (dist.parse_tag, "ta g", "TagInvalidChar", 2),
        (dist.parse_tag, "tag/path", "TagInvalidChar", 3),
        (dist.parse_algorithm, "", "AlgorithmMissing", 0),
        (
            dist.parse_digest,
            DIGEST[:-1],
            "OciRegisteredAlgorithmWrongDigestLength",
            70,
        ),
        (dist.parse_digest, "sha256:" + "A" * 64, "OciRegisteredDigestInvalidChar", 7),
    ],
)
def test_component_errors_have_kind_and_index(parser, src, kind, index):
    with pytest.raises(dist.ParseError) as excinfo:
        parser(src)

    assert excinfo.value.kind == kind
    assert excinfo.value.index == index


def test_error_index_for_non_ascii_input():
    with pytest.raises(dist.ParseError) as excinfo:
        dist.parse("répo")

    assert excinfo.value.kind == "HostOrPathInvalidChar"
    # 'é' occupies bytes 1-2; the parser reports the first invalid byte.
    assert excinfo.value.index == 1


def test_empty_reference_is_rejected():
    with pytest.raises(dist.ParseError) as excinfo:
        dist.parse("")
    assert excinfo.value.kind == "RefMissing"
    assert excinfo.value.index == 0


@pytest.mark.parametrize(
    ("obj", "attribute"),
    [
        (dist.parse("a/b"), "name"),
        (dist.parse_canonical("host.com/a@" + DIGEST), "digest"),
        (dist.Digest(DIGEST), "algorithm"),
        (dist.Domain("host.com"), "host"),
        (dist.Path("a/b"), "parts"),
    ],
)
def test_classes_are_immutable(obj, attribute):
    with pytest.raises(AttributeError):
        setattr(obj, attribute, None)


def test_component_equality_and_hashing():
    pairs = [
        (dist.Digest(DIGEST), dist.Digest(DIGEST)),
        (dist.Algorithm("sha256"), dist.Algorithm("sha256")),
        (dist.Encoded("deadbeef"), dist.Encoded("deadbeef")),
        (dist.Domain("localhost:5000"), dist.Domain("localhost:5000")),
        (dist.Host("[::1]"), dist.Host("[::1]")),
        (dist.Path("a/b"), dist.Path("a/b")),
        (dist.Tag("v1"), dist.Tag("v1")),
    ]
    for left, right in pairs:
        assert left == right
        assert hash(left) == hash(right)


def test_repr_matches_the_source_string():
    cases = [
        (dist.parse("a/b"), 'ImgRef("a/b")'),
        (
            dist.parse_canonical("host.com/a@" + DIGEST),
            f'CanonicalImgRef("host.com/a@{DIGEST}")',
        ),
        (dist.Digest(DIGEST), f'Digest("{DIGEST}")'),
        (dist.Algorithm("sha256+x"), 'Algorithm("sha256+x")'),
        (dist.Encoded("abcd"), 'Encoded("abcd")'),
        (dist.Domain("host.com:5000"), 'Domain("host.com:5000")'),
        (dist.Host("[::1]"), 'Host("[::1]")'),
        (dist.Path("a/b"), 'Path("a/b")'),
        (dist.Tag("v1"), 'Tag("v1")'),
    ]
    for obj, expected in cases:
        assert repr(obj) == expected
