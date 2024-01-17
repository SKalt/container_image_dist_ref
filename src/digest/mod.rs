//! # Digest
//!
//! Parsers for digest strings according to either the [OCI image spec][image-spec]
//! or the grammar used by [`distribution/reference`][ref].
//! These grammars differ slightly:

// {{{sh
//    echo; cat ../../grammars/digest.diff | sed 's#^#//!#g'; printf '\n// '
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
//! [image-spec]: https://github.com/opencontainers/image-spec/blob/v1.0.2/descriptor.md#digests
//! [ref]: https://github.com/distribution/reference/blob/v0.5.0/reference.go#L24

pub mod algorithm;
pub mod encoded;

use crate::{
    err,
    span::{IntoOption, Lengthy, Long},
};

use self::{
    algorithm::{AlgorithmSpan, AlgorithmStr},
    encoded::{EncodedSpan, EncodedStr},
};
type Error = err::Error<Long>;
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

#[derive(Clone, Copy, PartialEq, Eq)]
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

impl Compliance {
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
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) struct DigestSpan<'src> {
    algorithm: AlgorithmSpan<'src>,
    encoded: EncodedSpan<'src>,
    compliance: Compliance,
}

const MAX_USIZE: usize = Long::MAX as usize;
impl<'src> DigestSpan<'src> {
    pub(crate) fn new(src: &'src str) -> Result<Self, Error> {
        match src.len() {
            0 => return Ok(Self::none()),
            1..=MAX_USIZE => {} // ok length
            _ => return Error::at(Long::MAX, err::Kind::DigestTooLong).into(),
        }
        let (algorithm, compliance) = AlgorithmSpan::new(src)?;
        let mut len = algorithm.short_len();
        let rest = &src[len as usize..];
        len = match rest.bytes().next() {
            Some(b':') => len
                .checked_add(1)
                .ok_or_else(|| Error::at(len.into(), err::Kind::DigestTooLong)),
            None => Error::at(len.into(), err::Kind::AlgorithmNoMatch).into(),
            _ => Error::at(len.into(), err::Kind::AlgorithmInvalidChar).into(),
        }?;
        let rest = &src[len as usize..];
        let (encoded, compliance) = EncodedSpan::new(rest, compliance)?;

        {
            let rest = &src[len as usize..];
            let algorithm = AlgorithmStr::from_span(src, algorithm);
            let encoded = EncodedStr::from_span(rest, encoded);
            encoded.validate_algorithm(&algorithm, compliance)?;
        }

        Ok(Self {
            algorithm,
            encoded,
            compliance,
        })
    }
}
impl Lengthy<'_, Long> for DigestSpan<'_> {
    fn short_len(&self) -> Long {
        self.algorithm
            .into_option()
            .map(|algo| algo.short_len() as Long + 1 + self.encoded.short_len())
            .unwrap_or(0)
    }
}

impl IntoOption for DigestSpan<'_> {
    fn is_some(&self) -> bool {
        self.algorithm.is_some() && self.encoded.is_some()
    }

    fn none() -> Self {
        Self {
            algorithm: AlgorithmSpan::none(),
            encoded: EncodedSpan::none(),
            compliance: Compliance::Universal,
        }
    }
}
pub struct DigestStr<'src> {
    src: &'src str,
    span: DigestSpan<'src>,
}

impl<'src> DigestStr<'src> {
    pub fn new(src: &'src str) -> Result<Self, Error> {
        let span = DigestSpan::new(src)?;
        Ok(Self { src, span })
    }
    pub(crate) fn from_span(src: &'src str, span: DigestSpan<'src>) -> Self {
        Self { src, span }
    }
    pub fn src(self) -> &'src str {
        self.src
    }
    pub fn algorithm(&self) -> AlgorithmStr<'src> {
        AlgorithmStr::from_span(self.src, self.span.algorithm)
    }
    pub fn encoded(&self) -> EncodedStr<'src> {
        EncodedStr::from_span(
            &self.src[self.span.algorithm.len() + 1..],
            self.span.encoded,
        )
    }
    pub fn compliance(&self) -> Compliance {
        self.span.compliance
    }
}
