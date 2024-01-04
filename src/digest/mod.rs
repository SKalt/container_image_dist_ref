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
use self::{algorithm::Algorithm, encoded::Encoded};
use super::parse::{Compliance, Parse};

pub struct Digest<'src> {
    pub algorithm: Algorithm<'src>,
    pub encoded: Encoded<'src>,
}

impl<'src> Digest<'src> {
    pub(crate) fn from_parts_unchecked(algorithm: &'src str, encoded: &'src str) -> Self {
        Self {
            algorithm: Algorithm::new_unchecked(algorithm),
            encoded: Encoded::new_unchecked(encoded),
        }
    }
    pub fn from_parts(algorithm: &'src str, encoded: &'src str) -> Result<Self, Parse> {
        let algorithm = Algorithm::from_exact_match(algorithm)?;
        let encoded = Encoded::from_exact_match(encoded)?;
        Ok(Self { algorithm, encoded })
    }
    pub fn new(digest: &'src str) -> Result<Self, Parse> {
        let mut src = digest;
        let algorithm = Algorithm::from_prefix(src)?;
        src = &src[algorithm.len()..];
        match src.chars().next() {
            Some(':') => src = &src[1..],
            _ => {
                return Err(Parse {
                    len: algorithm.len() as u8,
                    compliance: Compliance::Uncompliant,
                })
            }
        }
        let encoded = Encoded::from_exact_match(src)?;
        Ok(Self { algorithm, encoded })
    }
}

// impl<'src> TryFrom<&'src str> for Digest<'src> {
//     type Error = anyhow::Error;
//     fn try_from(digest: &'src str) -> Result<Self> {
//         Self::new(digest)
//     }
// }
