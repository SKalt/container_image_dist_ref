/// > A digest string MUST match the following grammar:
/// >
/// > ```ebnf
/// > digest                ::= algorithm ":" encoded
/// > algorithm             ::= algorithm-component (algorithm-separator algorithm-component)*
/// > algorithm-component   ::= [a-z0-9]+
/// > algorithm-separator   ::= [+._-]
/// > encoded               ::= [a-zA-Z0-9=_-]+
/// > ```
/// >
/// > -- https://github.com/opencontainers/image-spec/blob/v1.0.2/descriptor.md#digests
pub mod algorithm;
pub mod encoded;
use crate::{
    err::{self, Error},
    span::{IntoOption, SpanMethods, MAX_USIZE, U},
};

use self::{
    algorithm::{AlgorithmSpan, AlgorithmStr},
    encoded::{EncodedSpan, EncodedStr},
};

pub enum Standard {
    /// Though distribution/reference isn't officially a standard or specification
    /// as the de-facto reference implementation for references, we'll treat it as
    /// a standard.
    Distribution,

    /// As defined in the OCI image spec. // TODO: link
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
        match (self, standard) {
            (Compliance::Universal, _) => true,
            (Compliance::Oci, Standard::Oci) => true,
            (Compliance::Distribution, Standard::Distribution) => true,
            _ => false,
        }
    }
}

// Note: DigestSpan doesn't own a leading '@'; that's only implied when DigestSpan
// is part of a larger ReferenceSpan.
#[derive(Clone, Copy)]
pub(crate) struct OptionalDigestSpan<'src> {
    algorithm: AlgorithmSpan<'src>,
    encoded: EncodedSpan<'src>,
    compliance: Compliance,
}

impl<'src> OptionalDigestSpan<'src> {
    pub(crate) fn new(src: &'src str) -> Result<Self, Error> {
        match src.len() {
            0 => return Ok(Self::none()),
            MAX_USIZE => return Err(Error(err::Kind::DigestTooLong, U::MAX)),
            _ => {}
        }
        let (algorithm, compliance) = AlgorithmSpan::new(src)?;
        let mut len = algorithm.short_len();
        let rest = &src[len as usize..];
        len = match rest.bytes().next() {
            Some(b':') => Ok(len + 1),
            None => Err(Error(err::Kind::AlgorithmNoMatch, len)),
            _ => Err(Error(err::Kind::AlgorithmInvalidChar, len)),
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
impl SpanMethods<'_> for OptionalDigestSpan<'_> {
    fn short_len(&self) -> U {
        self.algorithm
            .into_option()
            .map(|present| present.short_len() + 1 + self.encoded.short_len())
            .unwrap_or(0)
    }
}

impl IntoOption for OptionalDigestSpan<'_> {
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
    pub src: &'src str,
    span: OptionalDigestSpan<'src>,
}

impl<'src> DigestStr<'src> {
    pub fn new(src: &'src str) -> Result<Self, Error> {
        let span = OptionalDigestSpan::new(src)?;
        Ok(Self { src, span })
    }
    // pub fn algorithm(&self) -> AlgorithmStr<'src> {
    //     AlgorithmStr::from_span(self.src, self.span.algorithm)
    // }
    // pub fn encoded(&self) -> EncodedStr<'src> {
    //     EncodedStr::from_span(self.src, self.span.encoded)
    // }
}
