//! > tag  := ":" [\w][\w.-]{0,127}
//! > port := ":" [0-9]+

use crate::span::{impl_span_methods_on_tuple, IntoOption, OptionalSpan, U};

#[derive(Debug)]
pub enum Error {
    // while length within a tag is limited to 127, the total length in an error
    // might be longer, so we can't pack the entire error into a single bit.
    TooLong(U),
    InvalidChar(U),
}
impl std::ops::Add<U> for Error {
    type Output = Self;
    fn add(self, rhs: U) -> Self {
        match self {
            Self::TooLong(len) => Self::TooLong(len + rhs),
            Self::InvalidChar(len) => Self::InvalidChar(len + rhs),
        }
    }
}

#[cfg_attr(test, derive(Debug))]
#[derive(Clone, Copy, PartialEq, Eq)]
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
            _ => Err(Error::InvalidChar(0)),
        }?;

        let mut kind = kind;
        loop {
            index = if index >= 127 {
                Err(Error::TooLong(index))
            } else if index as usize == src.len() - 1 {
                break; // end of string
            } else {
                Ok(index + 1)
            }?;
            let c = ascii[index as usize];
            kind = match c {
                b'a'..=b'z' | b'A'..=b'Z' | b'.' | b'-' => match kind {
                    Kind::Tag | Kind::Either => Ok(Kind::Tag),
                    Kind::Port => Err(Error::InvalidChar(index + 1)),
                },
                b'0'..=b'9' => match kind {
                    Kind::Tag | Kind::Port => Ok(kind),
                    Kind::Either => Ok(Kind::Port),
                },
                b'/' => {
                    index -= 1; // don't consume the slash
                    break;
                }
                _ => return Err(Error::InvalidChar(index + 1)),
            }?;
        }
        let len = index + 1;
        Ok(Self(OptionalSpan::new(len), kind))
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
