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

use super::algorithm::Algorithm;
use super::{Compliance, Error};
use crate::U;

fn encoding(s: &str, compliance: Compliance) -> Result<(U, Compliance), Error> {
    use Compliance::*;
    let mut len = 0;
    let mut compliance = compliance;

    for c in s.bytes() {
        len += 1;
        compliance = match c {
            b'a'..=b'f' | b'0'..=b'9' | b'A'..=b'F' => Ok(compliance), // hex digits are universally accepted
            b'g'..=b'z' | b'G'..=b'Z' | b'=' | b'_' | b'-' => {
                // non-hex ascii letters and [_-=] are acceptable according to
                // the OCI image spec but not distribution/reference
                if compliance != Distribution {
                    Ok(Oci)
                } else {
                    Err(Error::EncodingCompliance(len))
                }
            }
            _ => Err(Error::EncodingInvalidChar(len)),
        }?;
    }
    Ok((len, compliance))
}

pub struct Encoded<'src> {
    pub src: &'src str,
}
impl<'src> Encoded<'src> {
    #[inline(always)]
    pub fn len(&self) -> usize {
        self.src.len()
    }

    // no implementation of from_prefix(&str) because digests MUST terminate a
    // reference

    pub fn from_exact_match(src: &'src str, compliance: Compliance) -> Result<Self, Error> {
        encoding(src, compliance)?;
        Ok(Self { src })
    }
    fn validate_oci_algorithm(&self, algorithm: &Algorithm<'src>) -> Result<(), Error> {
        let mut parts = algorithm.parts();
        let first = parts.next().unwrap();
        match first {
            "sha256" | "sha512" => {
                if parts.count() != 0 {
                    return Err(Error::OciRegisteredAlgorithmTooManyParts(
                        algorithm.len().try_into().unwrap(),
                    ));
                }
                {
                    let mut i: U = 0;
                    for c in self.src.bytes() {
                        i += 1;
                        if !c.is_ascii_lowercase() || !c.is_ascii_hexdigit() {
                            return Err(Error::OciRegisteredAlgorithmNonLowerHexChar(
                                self.len().try_into().unwrap(),
                            ));
                        }
                    }
                }
                match first {
                    "sha256" => {
                        if self.src.len() == 64 {
                            Ok(())
                        } else {
                            Err(Error::OciRegisteredAlgorithmWrongLength(
                                self.len().try_into().unwrap(),
                            ))
                        }
                    }
                    "sha512" => {
                        if self.len() == 128 {
                            Ok(())
                        } else {
                            Err(Error::OciRegisteredAlgorithmWrongLength(
                                self.len().try_into().unwrap(),
                            ))
                        }
                    }
                    _ => unreachable!(),
                }
            }

            _ => Ok(()),
        }
    }
    fn validate_distribution(&self) -> Result<(), Error> {
        if self.len() < 32 {
            return Err(Error::EncodingTooShort(self.len().try_into().unwrap()));
        }
        // let mut i: U = 0;
        // for c in self.src.bytes() {
        //     i += 1;
        //     if !c.is_ascii_hexdigit() {
        //         return Err(Error::EncodingCompliance(i.try_into().unwrap()));
        //     }
        // }
        Ok(())
    }
    /// Note: `validate_algorithm` doesn't check character sets since that's handled
    /// by the `from_exact_match` constructor.
    pub fn validate_algorithm(
        &self,
        algorithm: &Algorithm<'src>,
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
                let oci = self.validate_oci_algorithm(algorithm).is_ok();
                let dist = self.validate_distribution().is_ok();
                match (oci, dist) {
                    (true, true) => Ok(Compliance::Universal),
                    (true, false) => Ok(Compliance::Oci),
                    (false, true) => Ok(Compliance::Distribution),
                    (false, false) => Err(Error::EncodingCompliance(0)),
                }
            }
        }
    }
}
