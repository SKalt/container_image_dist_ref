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
use crate::{
    err::{Error, Kind as ErrorKind},
    span::{impl_span_methods_on_tuple, IntoOption, OptionalSpan, Span, MAX_USIZE, U},
};

use ErrorKind::{
    EncodedInvalidChar, EncodedNonLowerHex, OciRegisteredAlgorithmTooManyParts,
    OciRegisteredAlgorithmWrongLength, OciRegisteredDigestInvalidChar,
};

#[derive(Clone, Copy)]
pub(crate) struct EncodedSpan<'src>(OptionalSpan<'src>);
impl_span_methods_on_tuple!(EncodedSpan);
impl<'src> EncodedSpan<'src> {
    pub(crate) fn new(src: &'src str, compliance: Compliance) -> Result<(Self, Compliance), Error> {
        use Compliance::*;
        let mut len = 0;
        let mut compliance = compliance;
        for c in src.bytes() {
            compliance = match c {
                b'a'..=b'f' | b'0'..=b'9' | b'A'..=b'F' => Ok(compliance), // hex digits are universally accepted
                b'g'..=b'z' | b'G'..=b'Z' | b'=' | b'_' | b'-' => {
                    // non-hex ascii letters and [_-=] are acceptable according to
                    // the OCI image spec but not distribution/reference
                    if compliance != Distribution {
                        Ok(Oci)
                    } else {
                        Err(Error(EncodedNonLowerHex, len))
                    }
                }
                _ => Err(Error(EncodedInvalidChar, len)),
            }?;
            len += 1;
        }
        debug_assert!(len as usize == src.len(), "must have consume all src");

        Ok((Self(OptionalSpan::new(len)), compliance))
    }
}

impl IntoOption for EncodedSpan<'_> {
    fn is_some(&self) -> bool {
        self.short_len() > 0
    }

    fn none() -> Self
    where
        Self: Sized,
    {
        Self(OptionalSpan::new(0))
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
        let (span, compliance) = EncodedSpan::new(src, compliance)?;
        Ok((Self(span.span_of(src)), compliance))
    }
    pub(crate) fn from_span(src: &'src str, span: EncodedSpan<'src>) -> Self {
        Self(span.span_of(src))
    }
    fn validate_oci_algorithm(&self, algorithm: &AlgorithmStr<'src>) -> Result<(), Error> {
        let mut parts = algorithm.parts();
        let first = parts.next().unwrap();
        match first {
            "sha256" | "sha512" => {
                if parts.count() != 0 {
                    return Err(Error(
                        OciRegisteredAlgorithmTooManyParts,
                        first.len().try_into().unwrap(),
                    ));
                }
                {
                    let mut i: U = 0;
                    for c in self.src().bytes() {
                        i += 1;
                        if !c.is_ascii_lowercase() || !c.is_ascii_hexdigit() {
                            return Err(Error(OciRegisteredDigestInvalidChar, i));
                        }
                    }
                }
                match (first, self.len()) {
                    ("sha256", 64) => Ok(()),
                    ("sha512", 128) => Ok(()),
                    (_, _) => Err(Error(
                        OciRegisteredAlgorithmWrongLength,
                        self.len().try_into().unwrap(),
                    )),
                }
            }

            _ => Ok(()),
        }
    }
    fn validate_distribution(&self) -> Result<(), Error> {
        match self.len() {
            0..=31 => Err(Error(ErrorKind::EncodingTooShort, self.short_len())),
            32..=MAX_USIZE => Ok(()),
            _ => Err(Error(ErrorKind::EncodingTooLong, self.short_len())),
        }
    }
    /// Note: `validate_algorithm` doesn't check character sets since that's handled
    /// by the `from_exact_match` constructor.
    pub fn validate_algorithm(
        &self,
        algorithm: &AlgorithmStr<'src>,
        compliance: Compliance,
    ) -> Result<Compliance, Error> {
        match compliance {
            Compliance::Oci => {
                self.validate_oci_algorithm(algorithm)?;
                Ok(Compliance::Oci)
            }
            Compliance::Distribution => {
                self.validate_distribution()?;
                Ok(Compliance::Distribution)
            }
            Compliance::Universal => {
                let dist = self.validate_distribution();
                let oci = self.validate_oci_algorithm(algorithm);
                match (oci.is_ok(), dist.is_ok()) {
                    (true, true) => Ok(Compliance::Universal),
                    (true, false) => Ok(Compliance::Oci),
                    (false, true) => Ok(Compliance::Distribution),
                    (false, false) => dist.map(|_| Compliance::Distribution), // distribution's error condition (len<32)  is more egregious
                }
            }
        }
    }
}
impl SpanMethods<'_> for EncodedStr<'_> {
    fn len(&self) -> usize {
        self.0.len()
    }
    fn short_len(&self) -> U {
        self.len().try_into().unwrap()
    }
}
