//! # Digest Algorithm
//!
//! There are two specifications for a digest algorithm string:
//! - the [OCI Image Spec](https://github.com/opencontainers/image-spec/blob/v1.0.2/descriptor.md#digests)
//! - [github.com/distribution/reference](https://github.com/distribution/reference/blob/v0.5.0/reference.go#L21-L23)
//!
//! The OCI spec is a subset of the distribution spec:
//!

// {{{sh
//    cat ../../grammars/digest_algorithm.diff | sed 's#^#//! #g';
//    printf '//! ```\n\n// '
// }}}{{{out skip=2

//! ```diff
//! --- distribution/reference
//! +++ opencontainers/image-spec
//!  algorithm            ::= component (separator component)*
//! -component            ::= [A-Za-z][A-Za-z0-9]*
//! +component            ::= [a-z0-9]+
//!  separator            ::= [+._-]
//! ```

// }}}

use core::num::NonZeroU8;

use super::Compliance;
use crate::{
    err,
    span::{impl_span_methods_on_tuple, nonzero, Lengthy, OptionallyZero, ShortLength},
};
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(super) struct AlgorithmSpan<'src>(ShortLength<'src>);
impl_span_methods_on_tuple!(AlgorithmSpan, u8, NonZeroU8);

type Error = err::Error<u16>;
use err::Kind::{
    AlgorithmInvalidChar, AlgorithmInvalidNumericPrefix, AlgorithmMissing, InvalidOciAlgorithm,
};
fn try_add(a: NonZeroU8, b: u8) -> Result<NonZeroU8, Error> {
    a.checked_add(b)
        .ok_or(Error::at(u8::MAX.into(), err::Kind::AlgorithmTooLong))
}

/// While there's no specification for the max length of an algorithm string,
/// 255 characters is a reasonable upper bound.
pub const MAX_LEN: u8 = u8::MAX;

impl<'src> AlgorithmSpan<'src> {
    pub(crate) fn new(src: &'src str) -> Result<(Self, Compliance), Error> {
        let (mut len, mut compliance) =
            component(src, Compliance::Universal)?.ok_or(Error::at(0, AlgorithmMissing))?;
        let max_len = src.len().try_into().unwrap_or(MAX_LEN);
        loop {
            if u8::from(len) >= max_len {
                break;
            } else {
                match src.as_bytes()[u8::from(len) as usize] {
                    b':' => break,
                    b'+' | b'.' | b'_' | b'-' => {
                        len = try_add(len, 1)?; // consume the separator
                    }
                    _ => return Error::at(u8::from(len).into(), AlgorithmInvalidChar).into(),
                }
            }
            let (component_len, component_compliance) =
                component(&src[len.as_usize()..], compliance)?
                    .ok_or(Error::at(u8::from(len).into(), AlgorithmMissing))?;
            len = try_add(len, component_len.into())?;
            compliance = component_compliance; // narrow compliance from Universal -> (Oci | Distribution)
        }
        Ok((Self(ShortLength::from_nonzero(len)), compliance))
    }
    fn from_exact_match(src: &'src str) -> Result<(Self, Compliance), Error> {
        let (span, compliance) = Self::new(src)?;
        if span.len() == src.len() {
            Ok((span, compliance))
        } else {
            Error::at(span.short_len().upcast().into(), AlgorithmMissing).into()
        }
    }
}

/// The algorithm section of a digest.
/// ```rust
/// use container_image_dist_ref::digest::{
///     algorithm::AlgorithmStr, Compliance, Standard
/// };
/// let (algorithm, compliance) = AlgorithmStr::new("sha256").unwrap();
/// assert_eq!(algorithm.to_str(), "sha256");
/// assert_eq!(compliance, Compliance::Universal);
/// assert_eq!(algorithm.compliance(), Compliance::Universal);
/// assert!(compliance.compliant_with(Standard::Oci));
/// assert!(compliance.compliant_with(Standard::Distribution));
///
/// let (algorithm, _) = AlgorithmStr::new("a+b").unwrap();
/// assert_eq!(algorithm.to_str(), "a+b");
/// assert_eq!(algorithm.parts().collect::<Vec<_>>(), vec!["a", "b"]);
/// ```
pub struct AlgorithmStr<'src>(&'src str);
impl<'src> AlgorithmStr<'src> {
    #[allow(missing_docs)]
    #[inline]
    pub fn to_str(&self) -> &'src str {
        self.0
    }
    #[allow(missing_docs)]
    pub fn len(&self) -> usize {
        self.to_str().len()
    }
    #[allow(missing_docs)]
    pub fn is_empty(&self) -> bool {
        self.to_str().is_empty()
    }
    /// Parse an algorithm from the start of the string. Parsing may not consume the entire string
    /// if it reaches a valid stopping point, i.e. `:`.
    pub fn new(src: &'src str) -> Result<(Self, Compliance), Error> {
        let (span, compliance) = AlgorithmSpan::new(src)?;
        Ok((Self(span.span_of(src)), compliance))
    }
    /// checks that the entire source string is parsed.
    pub fn from_exact_match(src: &'src str) -> Result<(Self, Compliance), Error> {
        let (span, compliance) = AlgorithmSpan::from_exact_match(src)?;
        Ok((Self(span.span_of(src)), compliance))
    }
    pub(super) fn from_span(src: &'src str, span: AlgorithmSpan<'src>) -> Self {
        Self(span.span_of(src))
    }
    /// Split the algorithm string into its components separated by `+`, `.`, `_`, or `-`.
    pub fn parts(&self) -> impl Iterator<Item = &str> {
        self.to_str().split(|c| is_separator(c as u8))
    }
    /// Whether the algorithm is compliant with the OCI or distribution/reference specifications.
    pub fn compliance(&self) -> Compliance {
        let mut bytes = self.to_str().bytes();
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
fn component(src: &str, compliance: Compliance) -> Result<Option<(NonZeroU8, Compliance)>, Error> {
    use Compliance::*;
    let mut bytes = src.bytes();
    let compliance = match bytes.next() {
        None => return Ok(None),
        Some(b'a'..=b'z') => Ok(compliance), // universally compatible first character
        Some(b'0'..=b'9') => {
            // acceptable according to OCI spec, but not distribution/reference
            //  but not the OCI image spec
            if compliance == Distribution {
                // this is not a valid OCI algorithm
                Err(AlgorithmInvalidNumericPrefix)
            } else {
                Ok(Oci)
            }
        }
        Some(b'A'..=b'Z') => {
            // acceptable according to distribution/reference
            // but not the OCI image spec
            if compliance == Oci {
                // this is not a valid OCI algorithm
                Err(InvalidOciAlgorithm)
            } else {
                Ok(Distribution)
            }
        }
        _ => Err(AlgorithmInvalidChar),
    }
    .map_err(|kind| Error::at(0, kind))?;

    let mut len = nonzero!(u8, 1);
    for c in bytes {
        #[cfg(debug_assertions)]
        let _c = c as char;
        match c {
            b'a'..=b'z' | b'0'..=b'9' => Ok(()),
            b'A'..=b'Z' => {
                match compliance {
                    // uppercase letters are acceptable according to distribution/reference
                    Distribution | Universal => Ok(()),
                    // but not the OCI image spec
                    Oci => Err(InvalidOciAlgorithm),
                }
            }
            b':' | b'+' | b'.' | b'_' | b'-' => break,
            _ => Err(AlgorithmInvalidChar),
        }
        .map_err(|kind| Error::at(u8::from(len).into(), kind))?;

        len = len
            .checked_add(1)
            .ok_or(Error::at(u8::from(len).into(), err::Kind::AlgorithmTooLong))?;
    }

    Ok(Some((len, compliance)))
}
