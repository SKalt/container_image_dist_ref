"""Parse and validate docker/OCI container image references.

This package wraps the Rust `container_image_dist_ref` crate. Parsing is
validated against `distribution/reference`, the de-facto reference
implementation.
"""

from ._native import (
    Algorithm,
    CanonicalImgRef,
    Digest,
    Domain,
    Encoded,
    Host,
    ImgRef,
    ParseError,
    Path,
    Tag,
    __version__,
    parse,
    parse_algorithm,
    parse_canonical,
    parse_digest,
    parse_domain,
    parse_encoded,
    parse_host,
    parse_path,
    parse_tag,
)

__all__ = [
    "Algorithm",
    "CanonicalImgRef",
    "Digest",
    "Domain",
    "Encoded",
    "Host",
    "ImgRef",
    "ParseError",
    "Path",
    "Tag",
    "__version__",
    "parse",
    "parse_algorithm",
    "parse_canonical",
    "parse_digest",
    "parse_domain",
    "parse_encoded",
    "parse_host",
    "parse_path",
    "parse_tag",
]
