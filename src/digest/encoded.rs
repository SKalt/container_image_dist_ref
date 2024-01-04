//! # Encoded
//! There are two specifications for the encoded portion of a digest string:
//! - the [OCI Image Spec][image-spec]
//! - [github.com/distribution/reference][ref]
//!
//! The distribution/reference implementation is a subset of the OCI spec:
//!
//! ```diff
//! --- a/distribution/reference
//! +++ b/opencontainers/image-spec
//! -   hex     ::= [0-9a-fA-F]{32,}
//! +   encoded ::= [a-zA-Z0-9=_-]+
//! ```
//!
//! [image-spec]: https://github.com/opencontainers/image-spec/blob/v1.0.2/descriptor.md#digests
//! [ref]: https://github.com/distribution/reference/blob/v0.5.0/reference.go#L24
//!

use super::algorithm::Algorithm;
use crate::parse::{as_result, Compliance, Parse};

fn parse(s: &str) -> Parse {
    use Compliance::*;
    let mut result = Parse {
        len: 0,
        compliance: Universal,
    };
    for c in s.chars() {
        match c {
            'a'..='f' | 'A'..='F' | '0'..='9' => result.len += 1,
            'g'..='z' | 'G'..='Z' | '=' | '_' | '-' => {
                // acceptable according to the OCI image spec but
                // not distribution/reference
                if result.compliance == Compliance::Distribution {
                    result.compliance = Uncompliant;
                    break;
                }
                result.len += 1;
            }
            _ => {
                result.compliance = Uncompliant;
                break;
            }
        }
    }
    result
}

/// see  https://github.com/opencontainers/image-spec/blob/v1.0.2/descriptor.md#registered-algorithms
pub fn validate_oci_algorithm<'src>(algo: Algorithm<'src>, encoded: Encoded<'src>) {
    let mut parts = algo.parts();
    let first = parts.next().unwrap();
    match first {
        "sha256" | "sha512" => {
            assert!(
                parts.count() == 0,
                "too many parts for registered algorithm"
            );
            assert!(encoded
                .src
                .chars()
                .all(|c| c.is_lowercase() && c.is_ascii_hexdigit()));
            match first {
                "sha256" => {
                    assert!(encoded.len() == 64, "invalid digest: {}", encoded.src);
                }
                "sha512" => {
                    assert!(encoded.len() == 128, "invalid digest: {}", encoded.src);
                }
                _ => unreachable!(),
            }
        }

        _ => (),
    };
}

pub struct Encoded<'src> {
    pub src: &'src str,
}
impl<'src> Encoded<'src> {
    pub fn len(&self) -> usize {
        self.src.len()
    }
    pub(super) fn new_unchecked(src: &'src str) -> Self {
        Self { src }
    }

    pub fn from_exact_match(src: &'src str) -> Result<Self, Parse> {
        as_result(parse(src))?;
        Ok(Self { src })
    }
}
