//! > tag  := ":" [\w][\w.-]{0,127}
//! > port := ":" [0-9]+

use crate::{
    err::{self, Error},
    span::{IntoOption, Lengthy, Short, ShortLength},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Kind {
    /// a colon-prefixed span of digits. Can be either a port or a tag.
    Port,
    /// a colon-prefixed span of alphanumeric characters that must be a tag.
    Tag,
}
impl Kind {
    fn update(self, other: Self) -> Result<Self, ()> {
        match (self, other) {
            (Kind::Port, Kind::Port) | (Kind::Tag, Kind::Tag) => Ok(self),
            (Kind::Port, Kind::Tag) => Ok(Kind::Tag), // all ports are valid tags
            (Kind::Tag, Kind::Port) => Err(()),       // Kind::Tag is not compatible with Kind::Port
        }
    }
}
/// A span of characters that can be either a port or a tag.
/// Does NOT include the mandatory leading colon before either a port or a tag.
/// To accommodate the grammar's definition of a port as a nonzero numeric string,
/// the `.length` may be up to 255 characters, though tags are limited to 128 characters
/// after the colon.
#[derive(Clone, Copy)]
pub(crate) struct PortOrTagSpan<'src> {
    length: ShortLength<'src>,
    kind: Kind,
    first_tag_char: Short,
}
impl Lengthy<'_, Short> for PortOrTagSpan<'_> {
    #[inline(always)]
    fn short_len(&self) -> Short {
        self.length.short_len()
    }
}
impl<'src> IntoOption for PortOrTagSpan<'src> {
    #[inline(always)]
    fn is_some(&self) -> bool {
        self.short_len() > 0
    }
    #[inline(always)]
    fn none() -> Self {
        Self {
            length: 0.into(),
            kind: Kind::Port, // port is compatible with both ports and tags
            first_tag_char: 0,
        }
    }
}
struct State {
    len: Short,
    kind: Kind,
    first_tag_char: Short,
}
impl State {
    fn update_kind(&mut self, other: Kind) -> Result<(), Error> {
        match (self.kind, other) {
            (Kind::Port, Kind::Tag) => {
                self.first_tag_char = self.len;
                self.kind = Kind::Tag;
            } // all ports are valid tags
            _ => {}
        }
        self.kind = self
            .kind
            .update(other)
            .map_err(|_| Error::at(self.first_tag_char, err::Kind::PortInvalidChar))?;
        Ok(())
    }
    fn advance(&mut self) -> Result<(), Error> {
        if self.len >= 129 && self.kind == Kind::Tag {
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

impl<'src> PortOrTagSpan<'src> {
    #[inline(always)]
    pub(crate) fn span(&self) -> ShortLength<'src> {
        self.length
    }
    #[inline(always)]
    pub(crate) fn kind(&self) -> Kind {
        self.kind
    }
    pub(crate) fn narrow(&self, kind: Kind) -> Result<PortOrTagSpan<'src>, Error> {
        let kind = self
            .kind
            .update(kind)
            .map_err(|_| Error::at(self.first_tag_char, err::Kind::PortInvalidChar))?;
        Ok(PortOrTagSpan {
            length: self.length,
            kind,
            first_tag_char: self.first_tag_char,
        })
    }
    /// can match an empty span if the first character in src is a `/` or `@`
    pub(crate) fn new(src: &str, kind: Kind) -> Result<Self, Error> {
        let ascii = src.as_bytes();
        let mut bytes = src.bytes();
        let mut state = State {
            len: 0,
            kind,
            first_tag_char: 0, // only set on transition from port to tag
                               // and only used for providing an error index when
                               // trying to cast back from tag to port
        };

        // the first character after the colon must be alphanumeric or an underscore
        match bytes.next() {
            Some(b'0'..=b'9') => {
                // both ports and tags can have digits
                state.update_kind(state.kind)
            }
            Some(b'a'..=b'z') | Some(b'A'..=b'Z') | Some(b'_') => state.update_kind(Kind::Tag),
            None | Some(b'/') | Some(b'@') => Error::at(1, err::Kind::PortOrTagMissing).into(),
            _ => Error::at(1, err::Kind::PortOrTagInvalidChar).into(),
        }?;
        state.advance()?;

        for c in bytes {
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
        Ok(Self {
            length: state.len.into(),
            kind: state.kind,
            first_tag_char: state.first_tag_char,
        })
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
        should_parse_as("tag", Kind::Tag);
    }
    #[test]
    fn test_basic_port() {
        should_parse_as("1234", Kind::Port);
    }
}
