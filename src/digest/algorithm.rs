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
    err,
    span::{impl_span_methods_on_tuple, IntoOption, Lengthy, Long, Short, ShortLength},
};

use super::Compliance;
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) struct AlgorithmSpan<'src>(ShortLength<'src>);
impl_span_methods_on_tuple!(AlgorithmSpan, Short);

type Error = err::Error<Long>;
use err::Kind::{
    AlgorithmInvalidChar, AlgorithmInvalidNumericPrefix, AlgorithmNoMatch, InvalidOciAlgorithm,
};
fn try_add(a: Short, b: Short) -> Result<Short, Error> {
    a.checked_add(b)
        .ok_or(Error::at(Short::MAX.into(), err::Kind::AlgorithmTooLong))
}

impl<'src> AlgorithmSpan<'src> {
    pub(crate) fn new(src: &'src str) -> Result<(Self, Compliance), Error> {
        use Compliance::*;
        let initial_compliance = Universal;
        let (mut len, mut compliance) = component(src, initial_compliance)?;
        while let Some(next) = src[len as usize..].bytes().next() {
            if !is_separator(next) {
                break;
            }
            len = try_add(len, 1)?; // consume the separator
            let (component_len, component_compliance) =
                component(&src[len as usize..], compliance)?;
            len = try_add(len, component_len)?;
            compliance = component_compliance; // narrow compliance from Universal -> (Oci | Distribution)
        }
        Ok((Self(len.into()), compliance))
    }
    fn from_exact_match(src: &'src str) -> Result<(Self, Compliance), Error> {
        let (span, compliance) = Self::new(src)?;
        if span.len() == src.len() {
            Ok((span, compliance))
        } else {
            Error::at(span.short_len().into(), AlgorithmNoMatch).into()
        }
    }
}
impl IntoOption for AlgorithmSpan<'_> {
    fn is_some(&self) -> bool {
        self.short_len() > 0
    }

    fn none() -> Self {
        Self(0.into())
    }
}
pub struct AlgorithmStr<'src>(&'src str);
impl<'src> AlgorithmStr<'src> {
    pub fn src(&self) -> &'src str {
        self.0
    }
    pub fn len(&self) -> usize {
        self.src().len()
    }
    pub fn is_empty(&self) -> bool {
        self.src().is_empty()
    }
    pub fn from_prefix(src: &'src str) -> Result<(Self, Compliance), Error> {
        let (span, compliance) = AlgorithmSpan::new(src)?;
        Ok((Self(span.span_of(src)), compliance))
    }
    pub fn from_exact_match(src: &'src str) -> Result<(Self, Compliance), Error> {
        let (span, compliance) = AlgorithmSpan::from_exact_match(src)?;
        Ok((Self(span.span_of(src)), compliance))
    }
    pub(super) fn from_span(src: &'src str, span: AlgorithmSpan<'src>) -> Self {
        Self(span.span_of(src))
    }
    pub fn parts(&self) -> impl Iterator<Item = &str> {
        self.src().split(|c| is_separator(c as u8))
    }
    pub fn compliance(&self) -> Compliance {
        let mut bytes = self.src().bytes();
        match bytes.next().unwrap() {
            b'a'..=b'z' => {}
            b'0'..=b'9' => return Compliance::Oci,
            b'A'..=b'Z' => return Compliance::Distribution,
            _ => unreachable!("by construction, an AlgorithmStr may contain only [a-zA-Z0-9]"),
        };
        for c in bytes {
            match c {
                b'a'..=b'z' | b'0'..=b'9' => {}
                b'A'..=b'Z' => return Compliance::Distribution,
                _ => unreachable!("by construction, an AlgorithmStr may contain only [a-zA-Z0-9]"),
            }
        }
        Compliance::Universal
    }
}

/// match a single separator character: matching the regular expression /[+._-]/
fn is_separator(c: u8) -> bool {
    matches!(c, b'+' | b'.' | b'_' | b'-')
}

/// match an algorithm component and return the length of the match, along
/// with what standard(s) the component is compliant with.
fn component(src: &str, compliance: Compliance) -> Result<(Short, Compliance), Error> {
    use Compliance::*;
    if src.is_empty() {
        return Error::at(0, AlgorithmNoMatch).into();
    }

    let mut len: Short = 0;
    let compliance = match src.as_bytes()[len as usize] {
        b'a'..=b'z' => Ok(compliance), // universally compatible first character
        b'0'..=b'9' => {
            // acceptable according to OCI spec, but not distribution/reference
            //  but not the OCI image spec
            if compliance == Distribution {
                // this is not a valid OCI algorithm
                Error::at(len.into(), AlgorithmInvalidNumericPrefix).into()
            } else {
                Ok(Oci)
            }
        }
        b'A'..=b'Z' => {
            // acceptable according to distribution/reference
            // but not the OCI image spec
            if compliance == Oci {
                // this is not a valid OCI algorithm
                Error::at(len.into(), InvalidOciAlgorithm).into()
            } else {
                Ok(Distribution)
            }
        }
        _ => Error::at(len.into(), AlgorithmInvalidChar).into(),
    }?;
    len += 1;
    while (len as usize) < src.len() {
        let c = src.as_bytes()[len as usize];
        #[cfg(debug_assertions)]
        let _c = c as char;
        match c {
            b'a'..=b'z' | b'0'..=b'9' => Ok(()),
            b'A'..=b'Z' => {
                // acceptable according to distribution/reference
                // but not the OCI image spec
                if compliance == Oci {
                    // this is not a valid OCI algorithm
                    Error::at(len.into(), InvalidOciAlgorithm).into()
                } else {
                    Ok(())
                }
            }
            b':' | b'+' | b'.' | b'_' | b'-' => break,
            _ => Error::at(len.into(), AlgorithmInvalidChar).into(),
        }?;
        if len == Short::MAX {
            return Error::at(len.into(), err::Kind::AlgorithmTooLong).into();
        }
        len += 1;
    }
    Ok((len, compliance))
}
