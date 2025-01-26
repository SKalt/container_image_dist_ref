/*!
# Tag parsing

According to [the OCI distribution spec](https://github.com/opencontainers/distribution-spec/blob/v1.0.1/spec.md?plain=1#L64),
a tag is "a custom, human-readable manifest identifier".
According to [`distribution/reference`](https://github.com/distribution/reference/blob/v0.5.0/reference.go#L18),
a tag must have the following pattern:
*/
// {{{sh grep -E '^tag' ../grammars/reference.ebnf | sed -e 's/\s\+/ /g' }}}{{{out skip=3

/*
```ebnf
tag ::= [\w][\w.-]{0,127}
```
*/

// }}} skip=3
/*
Thus, tags can be up to 128 characters long.
*/
use core::num::NonZeroU8;

use crate::{
    ambiguous::port_or_tag::{Kind as TagKind, PortOrTagSpan},
    err,
    span::{impl_span_methods_on_tuple, nonzero, Lengthy, OptionallyZero, ShortLength},
};
/// The maximum length of a tag, as defined in [`distribution/reference`'s formal grammar](https://github.com/distribution/reference/blob/v0.5.0/reference.go#L18)
pub const MAX_LEN: NonZeroU8 = nonzero!(u8, 128_u8);

// we can index all errors with a u8 since the longest possible tag is 128 characters
type Error = err::Error<u8>;

/// max length = 128ch
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) struct TagSpan<'src>(ShortLength<'src>);
impl_span_methods_on_tuple!(TagSpan, u8, NonZeroU8);

impl<'src> TryFrom<PortOrTagSpan<'src>> for TagSpan<'src> {
    type Error = Error;
    fn try_from(ambiguous: PortOrTagSpan<'src>) -> Result<Self, Error> {
        if ambiguous.short_len() <= MAX_LEN {
            Ok(Self(ambiguous.span()))
        } else {
            Err(Error::at(MAX_LEN.upcast(), err::Kind::TagTooLong))
        }
    }
}

impl TagSpan<'_> {
    /// can match an empty span if the first character in `src` is a `/` or `@`
    pub(crate) fn new(src: &str) -> Result<Self, Error> {
        let span = PortOrTagSpan::new(src, TagKind::Tag).map_err(|e| {
            let kind = match e.kind() {
                err::Kind::PortOrTagInvalidChar => err::Kind::TagInvalidChar,
                err::Kind::PortOrTagMissing => err::Kind::TagMissing,
                // err::Kind::PortOrTagTooLong => err::Kind::TagTooLong,
                _ => e.kind(),
            };
            Error::at(e.index(), kind)
        })?;
        Ok(Self(span.span())) // safe since we parsed in TagKind::Tag mode
    }
}

/// A tag, not including any leading `:`.
/// Only guarantees that it contains a valid tag.
pub struct Tag<'src>(&'src str);
impl<'src> Tag<'src> {
    /// Parse a tag from a string.
    /// Returns an error if the tag is missing or invalid.
    /// Parsing may not consume the entire string if it encounters a valid stopping point,
    /// i.e. '@'.
    pub fn new(src: &'src str) -> Result<Self, Error> {
        let span = TagSpan::new(src)?;
        Ok(Self(span.span_of(src)))
    }
    #[allow(missing_docs)]
    #[inline]
    pub const fn to_str(&self) -> &'src str {
        self.0
    }
}
