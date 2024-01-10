//! > tag  := ":" [\w][\w.-]{0,127}
//! > port := ":" [0-9]+

use crate::{
    err::{self, Error},
    span::{impl_span_methods_on_tuple, IntoOption, OptionalSpan, U},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Kind {
    Either,
    /// a colon-prefixed span of digits
    Port,
    /// a colon-prefixed span of alphanumeric characters that must be a tag
    Tag,
}
#[derive(Clone, Copy)]
pub(crate) struct OptionalPortOrTag<'src>(pub(crate) OptionalSpan<'src>, pub(crate) Kind);
impl_span_methods_on_tuple!(OptionalPortOrTag);
impl<'src> IntoOption for OptionalPortOrTag<'src> {
    fn is_some(&self) -> bool {
        self.short_len() > 0
    }
    fn none() -> Self {
        Self(OptionalSpan::new(0), Kind::Either)
    }
}
impl<'src> OptionalPortOrTag<'src> {
    pub(crate) fn none() -> Self {
        Self(OptionalSpan::new(0), Kind::Either)
    }
    #[inline(always)]
    pub(crate) fn span(&self) -> OptionalSpan<'src> {
        self.0
    }
    #[inline(always)]
    pub(crate) fn kind(&self) -> Kind {
        self.1
    }
    pub(crate) fn new(src: &str, kind: Kind) -> Result<Self, Error> {
        let ascii = src.as_bytes();
        let mut index: U = match ascii.iter().next() {
            Some(b':') => Ok(0), // consume the starting colon
            Some(b'/') | None => return Ok(Self(OptionalSpan::new(0), kind)),
            _ => Err(Error(err::Kind::PortOrTagInvalidChar, 0)),
        }?;

        let mut kind = kind;
        loop {
            index = if index >= 127 {
                Err(Error(err::Kind::PortOrTagTooLong, index))
            } else if index as usize == src.len() - 1 {
                break; // end of string
            } else {
                Ok(index + 1)
            }?;
            let c = ascii[index as usize];
            kind = match c {
                b'a'..=b'z' | b'A'..=b'Z' | b'.' | b'-' => match kind {
                    Kind::Tag | Kind::Either => Ok(Kind::Tag),
                    Kind::Port => Err(Error(err::Kind::PortInvalidChar, index + 1)),
                },
                b'0'..=b'9' => match kind {
                    Kind::Tag | Kind::Port => Ok(kind),
                    Kind::Either => Ok(Kind::Port),
                },
                b'/' => {
                    index -= 1; // don't consume the slash
                    break;
                }
                _ => return Err(Error(err::Kind::PortOrTagInvalidChar, index + 1)),
            }?;
        }
        let len = index + 1;
        Ok(Self(OptionalSpan::new(len), kind))
    }
    pub(super) fn narrow(self, target: Kind, context: &'src str) -> Result<Self, Error> {
        match (self.kind(), target) {
            (_, Kind::Either) => Ok(Self(self.span(), Kind::Either)),
            (Kind::Either, _) => Ok(Self(self.span(), target)),
            (Kind::Port, Kind::Port) | (Kind::Tag, Kind::Tag) => Ok(self),

            (Kind::Port, Kind::Tag) => Ok(Self(self.span(), Kind::Tag)), // all ports are valid tags
            (Kind::Tag, Kind::Port) => Err(Error(
                err::Kind::PortInvalidChar,
                self.span_of(context)
                    .bytes()
                    .find(|b| !b.is_ascii_digit())
                    .unwrap() // safe since self.kind == Tag, which means there must be a non-digit char
                    .try_into()
                    .unwrap(), // safe since self.span_of(context) must be short
            )),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::span::SpanMethods;
    fn should_parse_as(src: &str, kind: Kind) {
        let tag = OptionalPortOrTag::new(src, kind);
        match tag {
            Ok(tag) => {
                assert_eq!(tag.span().span_of(src), src);
                assert_eq!(tag.kind(), kind);
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
