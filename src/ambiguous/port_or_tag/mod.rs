//! > tag  := ":" [\w][\w.-]{0,127}
//! > port := ":" [0-9]+

use crate::{
    err::{self, Error},
    span::{impl_span_methods_on_tuple, IntoOption, Lengthy, Short, ShortLength},
};

use super::Discriminant;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Kind {
    Either,
    /// a colon-prefixed span of digits
    Port,
    /// a colon-prefixed span of alphanumeric characters that must be a tag
    Tag,
}
#[derive(Clone, Copy)]
pub(crate) struct PortOrTag<'src> {
    pub(crate) short_length: ShortLength<'src>,
    pub(crate) kind: Kind,
    discriminant: Discriminant,
}
impl<'src> Lengthy<'src, Short> for PortOrTag<'src> {
    #[inline(always)]
    fn short_len(&self) -> Short {
        self.short_length.short_len()
    }
}
impl<'src> IntoOption for PortOrTag<'src> {
    fn is_some(&self) -> bool {
        self.short_len() > 0
    }
    fn none() -> Self {
        Self {
            short_length: ShortLength::none(),
            discriminant: Discriminant::none(),
            kind: Kind::Either,
        }
    }
}
impl<'src> PortOrTag<'src> {
    #[inline(always)]
    pub(crate) fn span(&self) -> ShortLength<'src> {
        self.short_length
    }
    pub(crate) fn new(src: &str, kind: Kind) -> Result<Self, Error> {
        if src.is_empty() {
            return Ok(Self::none());
        }
        let mut len = 0;
        let ascii = src.as_bytes();
        let mut discriminant: Option<Discriminant> = None;
        len += match ascii[len as usize] {
            b':' => Ok(1), // consume the starting colon
            b'/' | b'@' => {
                return Ok(Self {
                    short_length: 0.into(),
                    kind,
                    discriminant: discriminant.into(),
                })
            }
            _ => Err(Error(err::Kind::PortOrTagInvalidChar, len)),
        }?;

        let mut kind = kind;
        while (len <= 128) && (len as usize) < src.len() {
            let c = ascii[len as usize];
            #[cfg(test)]
            let _c = c as char;
            kind = match c {
                b'a'..=b'z' | b'A'..=b'Z' | b'.' | b'-' => {
                    discriminant |= Discriminant(len);
                    match kind {
                        Kind::Tag | Kind::Either => Ok(Kind::Tag),
                        Kind::Port => Err(Error(err::Kind::PortInvalidChar, len + 1)),
                    }
                }
                b'0'..=b'9' => match kind {
                    Kind::Tag | Kind::Port => Ok(kind),
                    Kind::Either => Ok(Kind::Port),
                },
                b'/' | b'@' => break,
                _ => return Err(Error(err::Kind::PortOrTagInvalidChar, len + 1)),
            }?;
            len += 1;
        }
        if len >= 128 {
            return Err(Error(err::Kind::PortOrTagTooLong, len));
        }
        Ok(Self {
            short_length: len.into(),
            kind,
            discriminant: discriminant.into(),
        })
    }
    pub(crate) fn narrow(self, target: Kind) -> Result<Self, Error> {
        let Self {
            kind,
            short_length,
            discriminant,
        } = self;
        match (kind, target) {
            (_, Kind::Either) => {
                debug_assert!(discriminant.is_none());
                debug_assert!(false, "don't narrow to Either");
                Ok(Self {
                    kind: Kind::Either,
                    short_length,
                    discriminant,
                })
            }
            (Kind::Either, _) => Ok(Self {
                short_length,
                kind: target,
                discriminant,
            }),
            (Kind::Port, Kind::Port) | (Kind::Tag, Kind::Tag) => Ok(self),
            (Kind::Port, Kind::Tag) => {
                debug_assert!(discriminant.is_none(), "all ports should be valid tags");
                Ok(Self {
                    kind: Kind::Tag,
                    short_length,
                    discriminant,
                })
            }
            (Kind::Tag, Kind::Port) => Error::at(discriminant.0, err::Kind::PortInvalidChar),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::span::Lengthy;
    fn should_parse_as(src: &str, kind: Kind) {
        let tag = PortOrTag::new(src, kind);
        match tag {
            Ok(tag) => {
                assert_eq!(tag.span().span_of(src), src);
                assert_eq!(tag.kind, kind);
            }
            Err(e) => panic!("failed to parse tag {src:?}: {:?}", e),
        }
    }

    #[test]
    fn test_basic_tag() {
        should_parse_as(":tag", Kind::Tag);
    }
    #[test]
    fn test_basic_port() {
        should_parse_as(":1234", Kind::Port);
    }
}
