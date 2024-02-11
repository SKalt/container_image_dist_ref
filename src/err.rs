//! # Errors
//! This module supplies the global error types for the crate.

// TODO: more docs
// since ErrorKind can fit 256 unique errors, use it for all non-ambiguous cases
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    // ambiguous::host_or_path ---------------------------------
    /// unable to match a host or path of length > 0. This is caused by
    /// attempting to parse an empty string.
    HostOrPathNoMatch,
    /// parsing the host or path section exceeded 255 characters.
    HostOrPathTooLong,
    HostOrPathInvalidChar,
    /// Caused by two incompatible path-component separators in a row, such as
    /// "..", "_.", "-.", etc.
    HostOrPathInvalidComponentEnd,
    // ambiguous::port_or_tag ----------------------------------
    /// caused by a colon immediately followed by EOF, "/", or "@"
    PortOrTagMissing,
    PortOrTagInvalidChar,
    // name ----------------------------------------------------------
    /// the name (including host, port, and path) is over 255 characters long.
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
    /// 129 or more characters after the ":".
    TagTooLong,
    TagInvalidChar,

    // digest::algorithm ----------------------------------------
    /// 0-length algorithm in an "algorithm:encoded" section detected
    // TODO: deprecate in favor of Option<Algorithm>
    AlgorithmNoMatch,
    /// If parsing in OCI-digest mode, uppercase letters are not allowed.
    InvalidOciAlgorithm,
    /// At least one algorithm component starts with a number, which is allowed
    /// by the OCI image spec but not distribution/reference. Then, the algorithm
    /// includes uppercase letters, which is allowed by distribution/reference
    /// but not the OCI image spec.
    AlgorithmInvalidNumericPrefix,
    /// Either a sha256 or sha512 algorithm was expected, but the digest was
    /// not 64 or 128 hex digits long.
    OciRegisteredAlgorithmWrongDigestLength,
    AlgorithmInvalidChar,
    /// 256 or more characters in the algorithm section.
    AlgorithmTooLong,
    // digest::encoded ------------------------------------------
    /// Nothing after the ":" in an "algorithm:encoded" section.
    EncodedNoMatch,
    /// a non-base64 character was encountered.
    EncodedInvalidChar,
    ///non-lower-hex characters are not allowed when parsing in `distribution/reference` mode
    EncodedNonLowerHex,
    OciRegisteredDigestInvalidChar,
    /// less than 32 characters in the encoded section of the digest
    EncodingTooShort,
    /// The digest was over 1024 bytes long. This is an arbitrary limit set in
    /// this repository. However, it is reasonable: 1024 hex digits can encode
    /// 4096-bit hashes, which is enough for an RSA key.
    EncodingTooLong,
    // reference ----------------------------------------
    /// empty string or non-canonical reference
    RefNoMatch,
}

/// The `Error` type contains an `err::Kind` and an index within the source string.
#[derive(Debug, Clone, Copy)]
pub struct Error<Size: Sized + Into<usize>>(Size, Kind);
impl From<Error<u8>> for Error<u16> {
    fn from(e: Error<u8>) -> Self {
        Self(e.0.into(), e.1)
    }
}

impl<Size> Error<Size>
where
    Size: Copy + Into<usize>,
{
    /// The byte index within the source string where the error occurred.
    #[inline]
    pub fn index(&self) -> Size {
        self.0
    }
    /// the kind of error
    #[inline]
    pub fn kind(&self) -> Kind {
        self.1
    }

    /// Create a new error at the given index
    pub(crate) const fn at(index: Size, kind: Kind) -> Self {
        Self(index, kind)
    }
}

impl<Int, Size> core::ops::Add<Int> for Error<Size>
where
    Size: core::ops::Add<Output = Size> + Into<usize>,
    Int: Into<Size>,
{
    type Output = Self;
    fn add(self, rhs: Int) -> Self {
        Self(self.0 + rhs.into(), self.1)
    }
}

impl<T, Size: Into<usize>> From<Error<Size>> for Result<T, Error<Size>> {
    #[inline(always)]
    fn from(value: Error<Size>) -> Self {
        Err(value)
    }
}
