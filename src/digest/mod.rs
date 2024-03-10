//! # Parsers for digest algorithms and encoded digest values
//!
//! Parsers for digest strings according to either the [OCI image spec](https://github.com/opencontainers/image-spec/blob/v1.0.2/descriptor.md#digests)
//! or the grammar used by [`distribution/reference`](https://github.com/distribution/reference/blob/v0.5.0/reference.go#L20-L24).
//! These grammars differ slightly:

// {{{sh
//    echo; cat ../../grammars/digest.diff | sed 's#^#//!#g';
//    printf '//! ```\n\n// '
// }}}{{{out skip=2

//!```diff

//!--- distribution/reference
//!+++ opencontainers/image-spec
//! digest               ::= algorithm ":" encoded
//! algorithm            ::= algorithm-component (algorithm-separator algorithm-component)*
//!-component            ::= [A-Za-z][A-Za-z0-9]*
//!+component            ::= [a-z0-9]+
//! separator            ::= [+._-]
//!-encoded              ::= [0-9a-fA-F]{32,} /* At least 128 bit digest value */
//!+encoded              ::= [a-zA-Z0-9=_-]+

// }}}
//! ```

pub mod algorithm;
pub mod encoded;

use core::num::NonZeroU16;

use crate::{
    err,
    span::{Lengthy, OptionallyZero},
};

use self::{
    algorithm::{Algorithm, AlgorithmSpan},
    encoded::{Encoded, EncodedSpan},
};
type Error = err::Error<u16>;
/// The standard or specification that a digest string must comply with. Used in
/// [`Compliance::compliant_with`].
pub enum Standard {
    /// Matching [0-9a-f]{32,} per distribution/reference.
    ///
    /// Though distribution/reference isn't officially a standard or specification
    /// as the de-facto reference implementation for references, we'll treat it as
    /// a standard.
    Distribution,

    /// As defined in [the OCI image spec](https://github.com/opencontainers/image-spec/blob/v1.0.2/descriptor.md#digests).
    Oci,
}

/// Whether a digest string is compliant with the OCI image spec, distribution/reference, or both.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Compliance {
    /// Not compliant with distribution/reference: at least one algorithm component
    /// starts with a number.
    Oci,
    /// Not compliant with OCI image spec: at least one letter is uppercase.
    Distribution,
    /// Compliant with both distribution/reference and OCI image spec.
    Universal,
    // non-compliance will always result in an error, so we don't need a variant
}
impl Default for Compliance {
    fn default() -> Self {
        Self::Universal
    }
}

impl Compliance {
    /// Checks whether a given compliance level is compliant with a given standard.
    pub fn compliant_with(self, standard: Standard) -> bool {
        matches!(
            (self, standard),
            (Compliance::Universal, _)
                | (Compliance::Oci, Standard::Oci)
                | (Compliance::Distribution, Standard::Distribution)
        )
    }
}

// Note: DigestSpan doesn't own a leading '@'; that's only implied when DigestSpan
// is part of a larger ReferenceSpan.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) struct DigestSpan<'src> {
    algorithm: AlgorithmSpan<'src>,
    encoded: EncodedSpan<'src>,
    compliance: Compliance,
}

impl<'src> DigestSpan<'src> {
    pub(crate) fn new(src: &'src str) -> Result<Self, Error> {
        let (algorithm, compliance) = AlgorithmSpan::new(src)?;
        let mut len = algorithm.short_len().widen();
        let rest = &src[len.as_usize()..];
        len = match rest.bytes().next() {
            Some(b':') => len.checked_add(1).ok_or(err::Kind::AlgorithmTooLong),
            None => Err(err::Kind::AlgorithmMissing),
            _ => Err(err::Kind::AlgorithmInvalidChar),
        }
        .map_err(|kind| Error::at(len.into(), kind))?;
        let rest = &src[len.as_usize()..];
        let (encoded, compliance) = EncodedSpan::new(rest, compliance).map_err(|e| e + len)?;

        {
            let rest = &src[len.as_usize()..];
            let algorithm = Algorithm::from_span(src, algorithm);
            let encoded = Encoded::from_span(rest, encoded);
            encoded.validate_algorithm(&algorithm, compliance)?;
        }

        Ok(Self {
            algorithm,
            encoded,
            compliance,
        })
    }
}
impl Lengthy<'_, u16, NonZeroU16> for DigestSpan<'_> {
    fn short_len(&self) -> NonZeroU16 {
        self.algorithm
            .short_len()
            .widen()
            .checked_add(1)
            .unwrap()
            .checked_add(self.encoded.short_len().upcast())
            .unwrap()
    }
    #[inline]
    fn len(&self) -> usize {
        self.short_len().as_usize()
    }
}

/// A parsed digest string. Includes the algorithm and encoded digest value,
/// along with information about whether the digest is compliant with the OCI image spec,
/// distribution/reference, or both.
pub struct Digest<'src> {
    src: &'src str,
    span: DigestSpan<'src>,
}

impl<'src> Digest<'src> {
    /// Parse a digest string NOT starting with a leading '@'. Parsing continues to the end of the string.
    pub fn new(src: &'src str) -> Result<Self, Error> {
        let span = DigestSpan::new(src)?;
        Ok(Self::from_span(&src[0..span.len()], span))
    }
    #[inline]
    pub(crate) fn from_span(src: &'src str, span: DigestSpan<'src>) -> Self {
        Self { src, span }
    }
    /// The original digest string, not including any leading '@'.
    #[inline]
    pub fn to_str(self) -> &'src str {
        self.src
    }
    /// The algorithm component of the digest string.
    pub fn algorithm(&self) -> Algorithm<'src> {
        Algorithm::from_span(self.src, self.span.algorithm)
    }
    /// The encoded digest value.
    pub fn encoded(&self) -> Encoded<'src> {
        Encoded::from_span(
            &self.src[self.span.algorithm.len() + 1..],
            self.span.encoded,
        )
    }
    /// Whether this digest is compliant with the OCI image spec, distribution/reference, or both.
    #[inline]
    pub fn compliance(&self) -> Compliance {
        self.span.compliance
    }
}
