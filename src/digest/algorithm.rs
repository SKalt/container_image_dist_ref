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
//!   digest      ::= algorithm ":" encoded
//!   algorithm   ::= component (separator component)*
//! - component   ::= [A-Za-z][A-Za-z0-9]
//! + component   ::= [a-z0-9]+
//!   separator   ::= [+._-]
//! ```
//!
//! [image-spec]: https://github.com/opencontainers/image-spec/blob/v1.0.2/descriptor.md#digests
//! [ref]: https://github.com/distribution/reference/blob/v0.5.0/reference.go#L21-L23
//!

use crate::{
    err::{self, Error},
    span::{impl_span_methods_on_tuple, IntoOption, OptionalSpan, U},
};

use super::Compliance;
#[derive(Clone, Copy)]
pub(super) struct AlgorithmSpan<'src>(OptionalSpan<'src>);
impl_span_methods_on_tuple!(AlgorithmSpan);
use err::Kind::{
    AlgorithmInvalidChar, AlgorithmInvalidNumericPrefix, AlgorithmNoMatch, InvalidOciAlgorithm,
};
impl<'src> AlgorithmSpan<'src> {
    pub(crate) fn new(src: &'src str) -> Result<(Self, Compliance), Error> {
        use Compliance::*;
        let initial_compliance = Universal;
        let (mut len, mut compliance) = component(src, initial_compliance)?;
        while let Some(next) = src[len as usize..].bytes().next() {
            if !is_separator(next) {
                break;
            }
            len += 1; // consume the separator
            let (component_len, component_compliance) =
                component(&src[len as usize..], compliance)?;
            len += component_len;
            compliance = component_compliance; // narrow compliance from Universal -> (Oci | Distribution)
        }
        Ok((Self(OptionalSpan::new(len)), compliance))
    }
    fn from_exact_match(src: &'src str) -> Result<(Self, Compliance), Error> {
        let (span, compliance) = Self::new(src)?;
        if span.len() == src.len() {
            Ok((span, compliance))
        } else {
            Err(Error(AlgorithmNoMatch, span.short_len()))
        }
    }
}
impl IntoOption for AlgorithmSpan<'_> {
    fn is_some(&self) -> bool {
        self.short_len() > 0
    }

    fn none() -> Self {
        Self(OptionalSpan::new(0))
    }
}
pub(crate) struct AlgorithmStr<'src>(&'src str);
impl<'src> AlgorithmStr<'src> {
    pub(crate) fn src(&self) -> &'src str {
        self.0
    }
    pub(crate) fn len(&self) -> usize {
        self.src().len()
    }
    fn from_prefix(src: &'src str) -> Result<(Self, Compliance), Error> {
        let (span, compliance) = AlgorithmSpan::new(src)?;
        Ok((Self(span.span_of(src)), compliance))
    }
    fn from_exact_match(src: &'src str) -> Result<(Self, Compliance), Error> {
        let (span, compliance) = AlgorithmSpan::from_exact_match(src)?;
        Ok((Self(span.span_of(src)), compliance))
    }
    pub(crate) fn from_span(src: &'src str, span: AlgorithmSpan<'src>) -> Self {
        Self(span.span_of(src))
    }
    pub fn parts(&self) -> impl Iterator<Item = &str> {
        self.src().split(|c| is_separator(c as u8))
    }
    pub fn compliance(&self) -> Compliance {
        let mut parts = self.parts();
        let first = parts.next().unwrap();
        match first {
            "sha256" | "sha512" => {
                if parts.count() != 0 {
                    Compliance::Oci
                } else {
                    Compliance::Distribution
                }
            }
            _ => Compliance::Universal,
        }
    }
}

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
        return Err(Error(AlgorithmNoMatch, 0));
    }
    debug_assert!(src.len() <= 256, "algorithm component too long"); // HACK: arbitrary limit

    let mut len = 0;
    let compliance = match src.as_bytes()[len as usize] {
        b'a'..=b'z' => Ok(compliance), // universally compatible first character
        b'0'..=b'9' => {
            // acceptable according to OCI spec, but not distribution/reference
            //  but not the OCI image spec
            if compliance == Distribution {
                // this is not a valid OCI algorithm
                Err(Error(AlgorithmInvalidNumericPrefix, len))
            } else {
                Ok(Oci)
            }
        }
        b'A'..=b'Z' => {
            // acceptable according to distribution/reference
            // but not the OCI image spec
            if compliance == Oci {
                // this is not a valid OCI algorithm
                Err(Error(InvalidOciAlgorithm, len))
            } else {
                Ok(Distribution)
            }
        }
        _ => Err(Error(AlgorithmInvalidChar, len)),
    }?;
    len += 1;
    while (len as usize) < src.len() {
        let c = src.as_bytes()[len as usize];
        #[cfg(test)]
        let _c = c as char;
        len = match c {
            b'a'..=b'z' | b'0'..=b'9' => Ok(len + 1),
            b'A'..=b'Z' => {
                // acceptable according to distribution/reference
                // but not the OCI image spec
                if compliance == Oci {
                    // this is not a valid OCI algorithm
                    Err(Error(InvalidOciAlgorithm, len))
                } else {
                    Ok(len + 1)
                }
            }
            b':' | b'+' | b'.' | b'_' | b'-' => break,
            _ => Err(Error(AlgorithmInvalidChar, len)),
        }?;
    }
    Ok((len, compliance))
}
