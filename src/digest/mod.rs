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
    span::{IntoOption, SpanMethods, U},
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
}
impl<'src> OptionalDigestSpan<'src> {
    pub(crate) fn short_len(&self) -> U {
        self.algorithm
            .into_option()
            .map(|present| present.short_len() + 1 + self.encoded.short_len())
            .unwrap_or(0)
    }
    pub(crate) fn len(&self) -> usize {
        self.short_len() as usize
    }
    pub(crate) fn new(src: &'src str) -> Result<(Self, Compliance), Error> {
        let (algorithm, compliance) = AlgorithmSpan::new(src)?;
        let mut len = match src[algorithm.len()..].bytes().next() {
            Some(b':') => Ok(algorithm.len() + 1),
            None => Err(Error(err::Kind::AlgorithmNoMatch, algorithm.short_len())),
            _ => Err(Error(
                err::Kind::AlgorithmInvalidChar,
                algorithm.short_len(),
            )),
        }?;
        let (encoded, compliance) = EncodedSpan::new(src, compliance)?;

        {
            let encoded = EncodedStr::from_span(&src[len..], encoded);
            let algorithm = AlgorithmStr::from_span(&src[..algorithm.len()], algorithm);
            encoded.validate_algorithm(&algorithm, compliance)?;
        }

        Ok((Self { algorithm, encoded }, compliance))
    }
}
impl IntoOption for OptionalDigestSpan<'_> {
    fn is_some(&self) -> bool {
        self.short_len() == 0
    }

    fn none() -> Self {
        Self {
            algorithm: AlgorithmSpan::none(),
            encoded: EncodedSpan::none(),
        }
    }
}
pub struct DigestStr<'src> {
    pub src: &'src str,
    span: OptionalDigestSpan<'src>,
}

impl<'src> DigestStr<'src> {
    pub fn new(src: &'src str) -> Result<(Self, Compliance), Error> {
        let (span, compliance) = OptionalDigestSpan::new(src)?;
        Ok((Self { src, span }, compliance))
    }
}
