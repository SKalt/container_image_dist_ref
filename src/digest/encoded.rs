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
use crate::span::{
    impl_span_methods_on_tuple, IntoOption, Lengthy, Long, LongLength, Short, MAX_USIZE,
};

use crate::err::Kind::{
    EncodedInvalidChar, EncodedNoMatch, EncodedNonLowerHex, EncodingTooLong, EncodingTooShort,
    OciRegisteredAlgorithmTooManyParts, OciRegisteredAlgorithmWrongLength,
    OciRegisteredDigestInvalidChar,
};
type Error = crate::err::Error<Long>;

#[derive(Clone, Copy)]
pub(crate) struct EncodedSpan<'src>(LongLength<'src>);
impl_span_methods_on_tuple!(EncodedSpan, Long);
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
                        Error::at(len, EncodedNonLowerHex)
                    }
                }
                _ => Error::at(len, EncodedInvalidChar),
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
        let (span, compliance) = EncodedSpan::new(src, compliance)?;
        if span.len() != src.len() {
            return Error::at(span.short_len().into(), EncodedNoMatch);
        }
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
                    return Error::at(
                        first.len().try_into().unwrap(),
                        OciRegisteredAlgorithmTooManyParts,
                    );
                }
                {
                    let mut i: Short = 0;
                    for c in self.src().bytes() {
                        i += 1;
                        if !c.is_ascii_lowercase() || !c.is_ascii_hexdigit() {
                            return Error::at(i.into(), OciRegisteredDigestInvalidChar);
                        }
                    }
                }
                match (first, self.len()) {
                    ("sha256", 64) => Ok(()),
                    ("sha512", 128) => Ok(()),
                    (_, _) => Error::at(
                        self.len().try_into().unwrap(),
                        OciRegisteredAlgorithmWrongLength,
                    ),
                }
            }

            _ => Ok(()),
        }
    }
    fn validate_distribution(&self) -> Result<(), Error> {
        match self.len() {
            0..=31 => Error::at(self.short_len().into(), EncodingTooShort),
            32..=MAX_USIZE => Ok(()),
            _ => Error::at(self.short_len().into(), EncodingTooLong),
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
impl Lengthy<'_, Long> for EncodedStr<'_> {
    fn len(&self) -> usize {
        self.0.len()
    }
    fn short_len(&self) -> Long {
        self.len().try_into().unwrap()
    }
}
