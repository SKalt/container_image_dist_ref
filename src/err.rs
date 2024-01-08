use std::fmt::Display;

use crate::span::U;

// since ErrorKind can fit 256 unique errors, use it for all non-ambiguous cases
#[derive(Debug, Clone, Copy)]
pub enum Kind {
    // domain::host --------------------------------------------
    HostNoMatch,
    HostComponentInvalidEnd,
    HostInvalidChar,
    HostTooLong,
    // domain::ipv6 -------------------------------------------
    Ipv6NoMatch,
    Ipv6TooLong,
    Ipv6BadColon,
    Ipv6TooManyHexDigits,
    Ipv6TooManyGroups,
    Ipv6TooFewGroups,
    Ipv6MissingClosingBracket,
    // domain::port --------------------------------------------
    Port,
    PortInvalidChar,
    PortTooLong,
    // path ----------------------------------------------------
    PathNoMatch,
    PathComponentInvalidEnd,
    PathInvalidChar,
    PathTooLong,
    // tag -----------------------------------------------------
    TagTooLong,
    TagInvalidChar,

    // digest::algorithm ----------------------------------------
    AlgorithmNoMatch,
    InvalidOciAlgorithm,
    /// At least one algorithm component starts with a number, which is allowed
    /// by the OCI image spec but not distribution/reference. Then, the algorithm
    /// includes uppercase letters, which is allowed by distribution/reference
    /// but not the OCI image spec.
    AlgorithmInvalidNumericPrefix,
    OciRegisteredAlgorithmInvalidChar,
    OciRegisteredAlgorithmTooManyParts,
    OciRegisteredAlgorithmWrongLength,
    AlgorithmInvalidChar,
    // digest::encoded ------------------------------------------
    EncodedNoMatch,
    EncodedInvalidChar,
    EncodedNonLowerHex,
    OciRegisteredDigestInvalidChar,
    EncodingTooShort,
    EncodingTooLong,
    // reference ----------------------------------------
    RefNoMatch,
    RefTooLong,
}

#[derive(Debug, Clone, Copy)]
pub struct Error(pub(crate) Kind, pub(crate) U);
impl std::ops::Add<U> for Error {
    type Output = Self;
    fn add(self, rhs: U) -> Self {
        Self(self.0, self.1 + rhs)
    }
}
