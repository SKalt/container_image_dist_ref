//! > tag  := ":" [\w][\w.-]{0,127}
//! > port := ":" [0-9]+

use crate::{
    err::{self, Error},
    span::{impl_span_methods_on_tuple, IntoOption, Lengthy, Short, ShortLength},
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
pub(crate) struct PortOrTagSpan<'src>(pub(crate) ShortLength<'src>, pub(crate) Kind);
impl_span_methods_on_tuple!(PortOrTagSpan, Short);
impl<'src> IntoOption for PortOrTagSpan<'src> {
    fn is_some(&self) -> bool {
        self.short_len() > 0
    }
    fn none() -> Self {
        Self(0.into(), Kind::Either)
    }
}
impl<'src> PortOrTagSpan<'src> {
    #[inline(always)]
    pub(crate) fn span(&self) -> ShortLength<'src> {
        self.0
    }
    #[inline(always)]
    pub(crate) fn kind(&self) -> Kind {
        self.1
    }
    /// can match an empty span if the first character in src is a `/` or `@`
    pub(crate) fn new(src: &str, kind: Kind) -> Result<Self, Error> {
        if src.is_empty() {
            return Ok(Self::none());
        }
        let mut len = 0;
        let ascii = src.as_bytes();
        len += match ascii[len as usize] {
            b':' => Ok(1), // consume the starting colon
            b'/' | b'@' => return Ok(Self(0.into(), kind)),
            _ => Err(Error(len, err::Kind::PortOrTagInvalidChar)),
        }?;

        let mut kind = kind;
        while (len <= 128) && (len as usize) < src.len() {
            let c = ascii[len as usize];
            #[cfg(test)]
            let _c = c as char;
            kind = match c {
                b'a'..=b'z' | b'A'..=b'Z' | b'.' | b'-' => match kind {
                    Kind::Tag | Kind::Either => Ok(Kind::Tag),
                    Kind::Port => Err(Error(len + 1, err::Kind::PortInvalidChar)),
                },
                b'0'..=b'9' => match kind {
                    Kind::Tag | Kind::Port => Ok(kind),
                    Kind::Either => Ok(Kind::Port),
                },
                b'/' | b'@' => break,
                _ => return Err(Error(len + 1, err::Kind::PortOrTagInvalidChar)),
            }?;
            len += 1;
        }
        if len >= 128 {
            return Err(Error(len, err::Kind::PortOrTagTooLong));
        }
        Ok(Self(len.into(), kind))
    }
    pub(super) fn narrow(self, target: Kind, context: &'src str) -> Result<Self, Error> {
        match (self.kind(), target) {
            (_, Kind::Either) => Ok(Self(self.span(), Kind::Either)),
            (Kind::Either, _) => Ok(Self(self.span(), target)),
            (Kind::Port, Kind::Port) | (Kind::Tag, Kind::Tag) => Ok(self),

            (Kind::Port, Kind::Tag) => Ok(Self(self.span(), Kind::Tag)), // all ports are valid tags
            (Kind::Tag, Kind::Port) => Err(Error(
                self.span_of(context)
                    .bytes()
                    .find(|b| !b.is_ascii_digit())
                    .unwrap() // safe since self.kind == Tag, which means there must be a non-digit char
                    .try_into()
                    .unwrap(), // safe since self.span_of(context) must be short
                err::Kind::PortInvalidChar,
            )),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::span::Lengthy;
    fn should_parse_as(src: &str, kind: Kind) {
        let tag = PortOrTagSpan::new(src, kind);
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
