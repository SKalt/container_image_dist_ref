//! # Algorithm
//! There are two specifications for a digest algorithm string:
//! - the [OCI Image Spec][image-spec]
//! - [github.com/distribution/reference][ref]
//!
//! The OCI spec is a subset of the distribution spec:
//!
//! ```diff
//! --- a/distribution/reference
//! +++ b/opencontainers/image-spec
//!   algorithm   ::= component (separator component)*
//! - component   ::= [A-Za-z][A-Za-z0-9]
//! + component   ::= [a-z0-9]+
//!   separator   ::= [+._-]
//! ```
//!
//! [image-spec]: https://github.com/opencontainers/image-spec/blob/v1.0.2/descriptor.md#digests
//! [ref]: https://github.com/distribution/reference/blob/v0.5.0/reference.go#L21-L23
//!

use crate::parse::{as_result, Compliance, Parse};
/// match a single separator character: matching the regular expression /[+._-]/
fn is_separator(c: char) -> bool {
    match c {
        '+' | '.' | '_' | '-' => true,
        _ => false,
    }
}

/// match an algorithm component and return the length of the match, along
/// with what standard(s) the component is compliant with.
fn component(src: &str) -> Parse {
    use Compliance::*;
    assert!(src.len() <= 256, "algorithm component too long"); // HACK: arbitrary limit
    let mut len = 0u8;
    let mut compliance = Universal;
    if let Some(first) = src.chars().next() {
        match first {
            '0'..='9' => {
                // acceptable according to OCI spec, but not distribution/reference
                //  but not the OCI image spec
                compliance = Oci;
            }
            _ => {}
        }
    } else {
        return Parse { len, compliance };
    }

    for c in src.chars() {
        match c {
            'a'..='z' | '0'..='9' => len += 1,
            'A'..='Z' => {
                // acceptable according to distribution/reference
                // but not the OCI image spec
                if compliance == Oci {
                    // this is not a valid OCI algorithm
                    compliance = Uncompliant;
                    return Parse { len, compliance };
                }
                len += 1;
            }
            _ => return Parse { len, compliance },
        }
    }
    Parse { len, compliance }
}

fn algo(src: &str) -> Parse {
    use Compliance::*;
    let mut result = component(src);
    if result.len == 0 {
        return result;
    }
    let mut len = result.len;
    let mut compliance = result.compliance;
    while len as usize <= src.len() {
        if let Some(sep) = src[len as usize..].chars().next() {
            if is_separator(sep) {
                len += 1;
                result = component(&src[len as usize..]);
                len += result.len;
                if result.compliance == Uncompliant {
                    compliance = Uncompliant;
                    break;
                }
            } else {
                break;
            }
        } else {
            break;
        }
    }
    Parse { len, compliance }
}

fn parse_prefix(src: &str) -> Result<Parse, Parse> {
    as_result(algo(src))
}

fn parse_exact_match(src: &str) -> Result<Parse, Parse> {
    let parsed = parse_prefix(src)?;
    if parsed.len as usize == src.len() {
        Ok(parsed)
    } else {
        Err(parsed)
    }
}

fn parts(valid_algo: &str) -> impl Iterator<Item = &str> {
    valid_algo.split(is_separator)
}

pub struct Algorithm<'src> {
    src: &'src str,
}
impl<'src> Algorithm<'src> {
    pub fn len(&self) -> usize {
        self.src.len()
    }
    pub(super) fn new_unchecked(src: &'src str) -> Self {
        Self { src }
    }
    /// extract an algorithm from the **start** of a string slice
    pub fn from_prefix(src: &'src str) -> Result<Self, Parse> {
        let prefix = parse_prefix(src)?;
        Ok(Self {
            src: &src[..prefix.len as usize],
        })
    }
    pub fn from_exact_match(src: &'src str) -> Result<Self, Parse> {
        let exact = parse_exact_match(src)?;
        Ok(Self {
            src: &src[..exact.len as usize],
        })
    }
    pub fn parts(&self) -> impl Iterator<Item = &str> {
        parts(&self.src)
    }
    pub fn compliance(&self) -> Compliance {
        algo(&self.src).compliance
    }
}
