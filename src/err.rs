use crate::span::{Long, Short};

// since ErrorKind can fit 256 unique errors, use it for all non-ambiguous cases
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Kind {
    // ambiguous::host_or_path ---------------------------------
    HostOrPathNoMatch,
    HostOrPathTooLong,
    HostOrPathInvalidChar,
    HostOrPathInvalidComponentEnd,
    // ambiguous::port_or_tag ----------------------------------
    PortOrTagMissing,
    PortOrTagTooLong,
    PortOrTagInvalidChar,
    // name ----------------------------------------------------------
    NameTooLong,
    // name::domain::host --------------------------------------------
    HostNoMatch,
    HostComponentInvalidEnd,
    HostInvalidChar,
    HostTooLong,
    // name::domain::ipv6 -------------------------------------------
    Ipv6NoMatch,
    Ipv6TooLong,
    Ipv6BadColon,
    Ipv6TooManyHexDigits,
    Ipv6TooManyGroups,
    Ipv6TooFewGroups,
    Ipv6MissingClosingBracket,
    // name::domain::port --------------------------------------------
    Port,
    PortInvalidChar,
    PortTooLong,
    /// an empty port was observed (like "host:/", or "host:" at the end of the string)
    PortMissing,
    // name::path ----------------------------------------------------
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
    OciRegisteredAlgorithmWrongDigestLength,
    AlgorithmInvalidChar,
    // digest::encoded ------------------------------------------
    DigestTooLong,
    EncodedNoMatch,
    EncodedInvalidChar,
    EncodedNonLowerHex,
    OciRegisteredDigestInvalidChar,
    EncodingTooShort,
    EncodingTooLong,
    // reference ----------------------------------------
    RefNoMatch,
}

#[derive(Debug, Clone, Copy)]
pub struct Error<Size = Short>(pub(crate) Size, pub(crate) Kind);
impl From<Error<Short>> for Error<Long> {
    fn from(e: Error<Short>) -> Self {
        Self(e.0.into(), e.1)
    }
}

impl<Size> Error<Size>
where
    Size: Copy,
{
    #[inline(always)]
    pub(crate) fn index(&self) -> Size {
        self.0
    }
    #[inline(always)]
    pub(crate) fn kind(&self) -> Kind {
        self.1
    }

    pub(crate) fn at(index: Size, kind: Kind) -> Self {
        Self(index, kind)
    }
}

impl<Int, Size> core::ops::Add<Int> for Error<Size>
where
    Size: core::ops::Add<Output = Size>,
    Int: Into<Size>,
{
    type Output = Self;
    fn add(self, rhs: Int) -> Self {
        Self(self.0 + rhs.into(), self.1)
    }
}

impl<T, Size> From<Error<Size>> for Result<T, Error<Size>> {
    #[inline(always)]
    fn from(value: Error<Size>) -> Self {
        Err(value)
    }
}
