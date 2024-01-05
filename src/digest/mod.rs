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
use crate::U;

use self::{algorithm::Algorithm, encoded::Encoded};

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
    // /// Not compliant with either distribution/reference or OCI image spec.
    // Uncompliant,
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

pub enum Error {
    /// At least one algorithm component starts with a number, which is allowed
    /// by the OCI image spec but not distribution/reference. Then, the algorithm
    /// includes uppercase letters, which is allowed by distribution/reference
    /// but not the OCI image spec.
    AlgorithmCase(U),
    AlgorithmComponentInvalidChar(U),
    EncodingTooShort(U),
    EncodingInvalidChar(U),
    OciRegisteredAlgorithmTooManyParts(U),
    OciRegisteredAlgorithmWrongLength(U),
    OciRegisteredAlgorithmNonLowerHexChar(U),
    /// the
    EncodingCompliance(U),
    NoMatch(U),
}

pub struct Digest<'src> {
    pub algorithm: Algorithm<'src>,
    pub encoded: Encoded<'src>,
}

impl<'src> Digest<'src> {
    pub fn from_parts(
        algorithm: &'src str,
        encoded: &'src str,
    ) -> Result<(Self, Compliance), Error> {
        let (algorithm, compliance) = Algorithm::from_exact_match(algorithm)?;
        let encoded = Encoded::from_exact_match(encoded, compliance)?;
        Ok((Self { algorithm, encoded }, compliance))
    }
    pub fn new(digest: &'src str) -> Result<(Self, Compliance), Error> {
        let mut src = digest;
        let (algorithm, compliance) = Algorithm::from_prefix(src)?;
        src = &src[algorithm.len()..];
        match src.bytes().next() {
            Some(b':') => src = &src[1..],
            None => return Err(Error::NoMatch(algorithm.len().try_into().unwrap())),
            _ => {
                return Err(Error::AlgorithmComponentInvalidChar(
                    algorithm.len().try_into().unwrap(),
                ))
            }
        }
        let encoded = Encoded::from_exact_match(src, compliance)?;
        encoded.validate_algorithm(&algorithm, compliance)?;
        Ok((Self { algorithm, encoded }, compliance))
    }
}
