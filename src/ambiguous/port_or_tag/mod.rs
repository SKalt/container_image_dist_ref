//! > tag  := ":" [\w][\w.-]{0,127}
//! > port := ":" [0-9]+

use crate::span::{impl_span_methods_on_tuple, IntoOption, OptionalSpan, U};

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
        let mut ascii = src.bytes();
        let mut len: U = match ascii.next() {
            Some(b':') => Ok(1),
            Some(b'/') | None => return Ok(Self(OptionalSpan::new(0), kind)),
            _ => Err(Error::InvalidChar(0)),
        }?;

        let mut kind = kind;
        for c in ascii {
            debug_assert!(len < 128, "128 <= {len} == len:");
            kind = match c {
                b'a'..=b'z' | b'A'..=b'Z' | b'.' | b'-' => match kind {
                    Kind::Tag => Ok(kind),
                    Kind::Either => Ok(Kind::Tag),
                    Kind::Port => Err(Error::InvalidChar(len + 1)),
                },
                b'0'..=b'9' => match kind {
                    Kind::Tag | Kind::Port => Ok(kind),
                    Kind::Either => Ok(Kind::Port),
                },
                b'/' => break,
                _ => return Err(Error::InvalidChar(len + 1)),
            }?;
            len += 1;
            if len > 127 {
                return Err(Error::TooLong(len));
            }
        }
        Ok(Self(OptionalSpan::new(len), kind))
    }
}
