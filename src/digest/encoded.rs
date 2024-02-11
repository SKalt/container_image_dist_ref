//! # Encoded
//! There are two specifications for the encoded portion of a digest string:
//! - the [OCI Image Spec](https://github.com/opencontainers/image-spec/blob/v1.0.2/descriptor.md#digests)
//! - [github.com/distribution/reference](https://github.com/distribution/reference/blob/v0.5.0/reference.go#L24)
//!
//! The distribution/reference implementation is a subset of the OCI spec:
//!

// {{{sh cat ../../grammars/digest_encoded.diff | sed 's#^#//! #g' }}}{{{out skip=2

//! ```diff
//! --- distribution/reference
//! +++ opencontainers/image-spec
//! -encoded  ::= [a-fA-F0-9]{32,} /* At least 128 bit digest value */
//! +encoded  ::= [a-zA-Z0-9=_-]+
//! ```

// }}} skip=2

use core::num::NonZeroU16;

use super::{algorithm::AlgorithmStr, Compliance};
use crate::span::{impl_span_methods_on_tuple, Lengthy, LongLength};

pub const MAX_LENGTH: u16 = 1024; // arbitrary but realistic limit

use crate::err::Kind::{
    EncodedInvalidChar, EncodedNoMatch, EncodedNonLowerHex, EncodingTooLong, EncodingTooShort,
    OciRegisteredAlgorithmWrongDigestLength, OciRegisteredDigestInvalidChar,
};
type Error = crate::err::Error<u16>;

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) struct EncodedSpan<'src>(LongLength<'src>);
impl_span_methods_on_tuple!(EncodedSpan, u16, NonZeroU16);
impl<'src> EncodedSpan<'src> {
    pub(crate) fn new(
        src: &'src str,
        compliance: Compliance,
    ) -> Result<Option<(Self, Compliance)>, Error> {
        use Compliance::*;
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
                        Err(EncodedNonLowerHex)
                    }
                }
                _ => Err(EncodedInvalidChar),
            }
            .map_err(|kind| Error::at(len, kind))?;
            if len == MAX_LENGTH {
                return Error::at(len, EncodingTooLong).into();
            }
            len += 1;
        }
        debug_assert!(len as usize == src.len(), "must have consume all src");

        Ok(LongLength::new(len).map(|length| (Self(length), compliance)))
    }
}

pub struct EncodedStr<'src>(&'src str);
impl<'src> EncodedStr<'src> {
    pub fn src(&self) -> &'src str {
        self.0
    }
    // no implementation of from_prefix(&str) because digests MUST terminate a
    // reference

    pub(crate) fn from_span(src: &'src str, span: EncodedSpan<'src>) -> Self {
        Self(span.span_of(src))
    }
    /// validates whether every ascii character is a lowercase hex digit
    fn is_lower_hex(&self) -> Result<(), Error> {
        self.src().bytes().enumerate().try_for_each(|(i, c)| {
            if matches!(c, b'a'..=b'f' | b'0'..=b'9') {
                Ok(())
            } else {
                Error::at(i.try_into().unwrap(), OciRegisteredDigestInvalidChar).into()
            }
        })
    }
    /// check that the encoded string is an appropriate hex length for the registered
    /// algorithms `sha256` and `sha512`.
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
            _ => Ok(()), // non-registered algorithm, so validation falls to the caller
        }
    }
    /// check that the encoded string is an appropriate length according to distribution/reference
    fn validate_distribution(&self) -> Result<(), Error> {
        const MAX: usize = MAX_LENGTH as usize;
        match self.len() {
            0..=31 => Error::at(self.short_len().into(), EncodingTooShort).into(),
            32..=MAX => Ok(()),
            _ => Error::at(self.short_len().into(), EncodingTooLong).into(),
        }
    }
    /// Validate the encoded string is compliant with an algorithm string (possibly a
    /// registered algorithm such as sha256 or sha512) and one or more of the OCI or
    /// distribution/reference specifications' constraints.
    pub fn validate_algorithm(
        &self,
        algorithm: &AlgorithmStr<'src>,
        compliance: Compliance,
    ) -> Result<Compliance, Error> {
        self.validate_registered_algorithms(algorithm)?;
        // Note: `validate_algorithm` doesn't check character sets since that's handled
        // by the `from_exact_match` constructor.
        match compliance {
            Compliance::Oci => Ok(Compliance::Oci),
            Compliance::Distribution => {
                self.validate_distribution()?;
                Ok(Compliance::Distribution)
            }
            Compliance::Universal => Ok(match self.validate_distribution() {
                Ok(_) => Compliance::Universal,
                Err(_) => Compliance::Oci,
            }),
        }
    }
}
impl Lengthy<'_, u16, NonZeroU16> for EncodedStr<'_> {
    #[inline]
    fn len(&self) -> usize {
        self.0.len()
    }
    #[inline]
    fn short_len(&self) -> NonZeroU16 {
        NonZeroU16::new(self.len().try_into().unwrap()).unwrap()
    }
}
