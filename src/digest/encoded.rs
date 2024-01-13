//! # Encoded
//! There are two specifications for the encoded portion of a digest string:
//! - the [OCI Image Spec][image-spec]
//! - [github.com/distribution/reference][ref]
//!
//! The distribution/reference implementation is a subset of the OCI spec:
//!
//! ```diff
//! --- a/distribution/reference
//! +++ b/opencontainers/image-spec
//! -   hex     ::= [a-fA-F0-9]{32,}
//! +   encoded ::= [a-zA-Z0-9=_-]+
//! ```
//!
//! [image-spec]: https://github.com/opencontainers/image-spec/blob/v1.0.2/descriptor.md#digests
//! [ref]: https://github.com/distribution/reference/blob/v0.5.0/reference.go#L24
//!

use super::algorithm::AlgorithmStr;
use super::Compliance;
use crate::span::{impl_span_methods_on_tuple, IntoOption, Lengthy, Long, LongLength, Short};
const MAX_LENGTH: usize = Long::MAX as usize;

use crate::err::Kind::{
    EncodedInvalidChar, EncodedNoMatch, EncodedNonLowerHex, EncodingTooLong, EncodingTooShort,
    OciRegisteredAlgorithmWrongDigestLength, OciRegisteredDigestInvalidChar,
};
type Error = crate::err::Error<Long>;

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) struct EncodedSpan<'src>(LongLength<'src>);
impl_span_methods_on_tuple!(EncodedSpan, Long);
impl<'src> EncodedSpan<'src> {
    pub(crate) fn new(src: &'src str, compliance: Compliance) -> Result<(Self, Compliance), Error> {
        use Compliance::*;
        if src.is_empty() {
            return Error::at(0, EncodedNoMatch).into();
        }
        let mut len = 0;
        let mut compliance = compliance;
        for c in src.as_bytes() {
            compliance = match c {
                b'a'..=b'f' | b'0'..=b'9' | b'A'..=b'F' => Ok(compliance), // hex digits are universally accepted
                b'g'..=b'z' | b'G'..=b'Z' | b'=' | b'_' | b'-' => {
                    // non-hex ascii letters and [_-=] are acceptable according to
                    // the OCI image spec but not distribution/reference
                    if compliance != Distribution {
                        Ok(Oci)
                    } else {
                        Error::at(len, EncodedNonLowerHex).into()
                    }
                }
                _ => Error::at(len, EncodedInvalidChar).into(),
            }?;
            len += 1;
        }
        debug_assert!(len as usize == src.len(), "must have consume all src");

        Ok((Self(len.into()), compliance))
    }
}

impl IntoOption for EncodedSpan<'_> {
    fn is_some(&self) -> bool {
        self.short_len() > 0
    }

    fn none() -> Self {
        Self(0.into())
    }
}
pub(crate) struct EncodedStr<'src>(&'src str);
impl<'src> EncodedStr<'src> {
    pub(crate) fn src(&self) -> &'src str {
        self.0
    }
    #[inline(always)]
    pub(crate) fn len(&self) -> usize {
        self.0.len()
    }

    // no implementation of from_prefix(&str) because digests MUST terminate a
    // reference

    pub(crate) fn from_exact_match(
        src: &'src str,
        compliance: Compliance,
    ) -> Result<(Self, Compliance), Error> {
        let (_, compliance) = EncodedSpan::new(src, compliance)?;
        Ok((Self(src), compliance))
    }
    pub(crate) fn from_span(src: &'src str, span: EncodedSpan<'src>) -> Self {
        Self(span.span_of(src))
    }
    fn is_lower_hex(&self) -> Result<(), Error> {
        let mut i: Short = 0;
        self.src().bytes().enumerate().try_for_each(|(i, c)| {
            if c.is_ascii_lowercase() && c.is_ascii_hexdigit() {
                Ok(())
            } else {
                Error::at(i.try_into().unwrap(), OciRegisteredDigestInvalidChar).into()
            }
        })
    }
    fn validate_registered_algorithms(&self, algorithm: &AlgorithmStr<'src>) -> Result<(), Error> {
        match algorithm.src() {
            "sha256" | "sha512" => {
                self.is_lower_hex()?;
                match (algorithm.src(), self.len()) {
                    ("sha256", 64) => Ok(()),
                    ("sha512", 128) => Ok(()),
                    (_, _) => Error::at(
                        self.len().try_into().unwrap(),
                        OciRegisteredAlgorithmWrongDigestLength,
                    )
                    .into(),
                }
            }
            _ => Ok(()), // non-registered algorithm
        }
    }

    fn validate_distribution(&self) -> Result<(), Error> {
        match self.len() {
            0..=31 => Error::at(self.short_len().into(), EncodingTooShort).into(),
            32..=MAX_LENGTH => Ok(()),
            _ => Error::at(self.short_len().into(), EncodingTooLong).into(),
        }
    }
    /// Note: `validate_algorithm` doesn't check character sets since that's handled
    /// by the `from_exact_match` constructor.
    pub fn validate_algorithm(
        &self,
        algorithm: &AlgorithmStr<'src>,
        compliance: Compliance,
    ) -> Result<Compliance, Error> {
        self.validate_registered_algorithms(algorithm)?;
        match compliance {
            Compliance::Oci => Ok(Compliance::Oci),
            Compliance::Distribution => {
                self.validate_distribution()?;
                Ok(Compliance::Distribution)
            }
            Compliance::Universal => match self.validate_distribution() {
                Ok(_) => Ok(Compliance::Universal),
                Err(_) => return Ok(Compliance::Oci),
            },
        }
    }
}
impl Lengthy<'_, Long> for EncodedStr<'_> {
    fn len(&self) -> usize {
        self.0.len()
    }
    fn short_len(&self) -> Long {
        self.len().try_into().unwrap()
    }
}
