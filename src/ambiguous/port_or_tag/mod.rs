//! > tag  := ":" [\w][\w.-]{0,127}
//! > port := ":" [0-9]+

use crate::{
    err::{self, Error},
    span::{impl_span_methods_on_tuple, IntoOption, Lengthy, Short, ShortLength},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Kind {
    /// a colon-prefixed span of digits. Can be either a port or a tag.
    Port,
    /// a colon-prefixed span of alphanumeric characters that must be a tag.
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
        Self(0.into(), Kind::Port) // port is compatible with both ports and tags
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
        let ascii = src.as_bytes();
        // safe since len is going from 0 -> 1
        match ascii.iter().next() {
            Some(b':') => Ok(()), // consume the starting colon
            None | Some(b'/') | Some(b'@') => return Ok(Self::none()),
            _ => Err(Error::at(0, err::Kind::PortOrTagInvalidChar)),
        }?;
        struct State {
            len: Short,
            kind: Kind,
            first_tag_char: Short,
        }
        impl State {
            fn update_kind(&mut self, other: Kind) -> Result<(), Error> {
                match (self.kind, other) {
                    (Kind::Port, Kind::Port) | (Kind::Tag, Kind::Tag) => Ok(()),
                    (Kind::Port, Kind::Tag) => {
                        self.first_tag_char = self.len;
                        self.kind = Kind::Tag;
                        Ok(())
                    } // all ports are valid tags
                    (Kind::Tag, Kind::Port) => {
                        Error::at(self.first_tag_char, err::Kind::PortInvalidChar).into()
                    } // Kind::Tag is not compatible with Kind::Port
                }
            }
            fn advance(&mut self) -> Result<(), Error> {
                if self.len > 128 && self.kind == Kind::Tag {
                    Error::at(self.len, err::Kind::TagTooLong).into()
                } else {
                    self.len = self
                        .len
                        .checked_add(1)
                        .ok_or(Error::at(self.len, err::Kind::PortTooLong))?;
                    Ok(())
                }
            }
        }
        let mut state = State {
            len: 1,
            kind,
            first_tag_char: Short::MAX, // <- since ports/tags are limited to 127 ch, this is 255 is a niche
        };

        while (state.len as usize) < src.len() {
            let c = ascii[state.len as usize];
            #[cfg(debug_assertions)]
            let _c = c as char;
            match c {
                b'0'..=b'9' => state.update_kind(state.kind), // both ports and tags can have digits
                b'a'..=b'z' | b'A'..=b'Z' | b'.' | b'-' | b'_' => state.update_kind(Kind::Tag),
                b'/' => state.update_kind(Kind::Port),
                b'@' => state.update_kind(Kind::Tag),
                _ => Error::at(state.len, err::Kind::PortOrTagInvalidChar).into(),
            }?;
            if c == b'/' || c == b'@' {
                break;
            }
            state.advance()?;
        }
        debug_assert!((state.len as usize) <= src.len());
        debug_assert!(if (state.len as usize) < src.len() {
            ascii[state.len as usize] == b'/' || ascii[state.len as usize] == b'@'
        } else {
            true
        });
        Ok(Self(state.len.into(), state.kind))
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
