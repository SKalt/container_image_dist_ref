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

use crate::U;

use super::{Compliance, Error};

/// match a single separator character: matching the regular expression /[+._-]/
fn is_separator(c: u8) -> bool {
    match c {
        b'+' | b'.' | b'_' | b'-' => true,
        _ => false,
    }
}

/// match an algorithm component and return the length of the match, along
/// with what standard(s) the component is compliant with.
fn component(src: &str, compliance: Compliance) -> Result<(U, Compliance), Error> {
    use Compliance::*;
    if src.len() == 0 {
        return Err(Error::NoMatch(0));
    }
    assert!(src.len() <= 256, "algorithm component too long"); // HACK: arbitrary limit
    let compliance = match src.bytes().next().unwrap() {
        b'a'..=b'z' => Ok(compliance), // universally compatible first character
        b'0'..=b'9' => {
            // acceptable according to OCI spec, but not distribution/reference
            //  but not the OCI image spec
            if compliance == Distribution {
                // this is not a valid OCI algorithm
                Err(Error::AlgorithmCase(0))
            } else {
                Ok(Oci)
            }
        }
        b'A'..=b'Z' => {
            // acceptable according to distribution/reference
            // but not the OCI image spec
            if compliance == Oci {
                // this is not a valid OCI algorithm
                Err(Error::AlgorithmCase(1))
            } else {
                Ok(Distribution)
            }
        }
        _ => Err(Error::AlgorithmComponentInvalidChar(0)),
    }?;
    let mut len = 0;
    for c in src.bytes() {
        len += 1;
        match c {
            b'a'..=b'z' | b'0'..=b'9' => {} // ok
            b'A'..=b'Z' => {
                // acceptable according to distribution/reference
                // but not the OCI image spec
                if compliance == Oci {
                    // this is not a valid OCI algorithm
                    return Err(Error::AlgorithmCase(len.try_into().unwrap()));
                }
            }
            _ => break,
        }
    }
    Ok((len, compliance))
}

fn algo(src: &str) -> Result<(U, Compliance), Error> {
    use Compliance::*;
    let initial_compliance = Universal;
    let (mut len, mut compliance) = component(src, initial_compliance)?;
    while let Some(next) = src[len as usize..].bytes().next() {
        if !is_separator(next) {
            break;
        }
        len += 1; // consume the separator
        let (component_len, component_compliance) = component(&src[len as usize..], compliance)?;
        len += component_len;
        compliance = component_compliance; // narrow compliance from Universal -> (Oci | Distribution)
    }
    Ok((len, compliance))
}

fn parse_prefix(src: &str) -> Result<(U, Compliance), Error> {
    algo(src)
}

fn parse_exact_match(src: &str) -> Result<(U, Compliance), Error> {
    let (len, compliance) = parse_prefix(src)?;
    if len as usize == src.len() {
        Ok((len, compliance))
    } else {
        Err(Error::NoMatch(len))
    }
}

fn parts(valid_algo: &str) -> impl Iterator<Item = &str> {
    valid_algo.split(|c| is_separator(c as u8))
}

pub struct Algorithm<'src> {
    src: &'src str,
}
impl<'src> Algorithm<'src> {
    pub fn len(&self) -> usize {
        self.src.len()
    }
    /// extract an algorithm from the **start** of a string slice
    pub fn from_prefix(src: &'src str) -> Result<(Self, Compliance), Error> {
        let (len, compliance) = parse_prefix(src)?;
        Ok((
            Self {
                src: &src[..len as usize],
            },
            compliance,
        ))
    }
    pub fn from_exact_match(src: &'src str) -> Result<(Self, Compliance), Error> {
        let (len, compliance) = parse_exact_match(src)?;
        Ok((
            Self {
                src: &src[..len as usize],
            },
            compliance,
        ))
    }
    pub fn parts(&self) -> impl Iterator<Item = &str> {
        parts(&self.src)
    }
    /// prefer hanging on to the compliance value from initial parsing via `from_prefix`
    /// or `from_exact_match` rather than re-parsing
    pub fn compliance(&self) -> Result<Compliance, Error> {
        let (_, compliance) = algo(&self.src)?;
        Ok(compliance)
    }
}
