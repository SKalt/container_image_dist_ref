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
            let (index, kind) = match e.kind() {
                err::Kind::PortOrTagInvalidChar => (e.index(), err::Kind::TagInvalidChar),
                err::Kind::PortOrTagMissing => (e.index(), err::Kind::TagMissing),
                err::Kind::PortInvalidChar => {
                    // Only a '/' can force a tag back to being a port; that's the
                    // character that makes this string an invalid tag.
                    let index = src
                        .as_bytes()
                        .iter()
                        .position(|&b| b == b'/')
                        .and_then(|i| u8::try_from(i).ok())
                        .unwrap_or(e.index());
                    (index, err::Kind::TagInvalidChar)
                }
                kind => (e.index(), kind),
            };
            Error::at(index, kind)
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

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    fn tag_error(src: &str) -> Error {
        match Tag::new(src) {
            Ok(tag) => panic!(
                "expected {src:?} to be rejected, parsed as {:?}",
                tag.to_str()
            ),
            Err(e) => e,
        }
    }

    #[test]
    fn tag_can_stop_at_digest() {
        assert_eq!(Tag::new("tag@algo:encoded").unwrap().to_str(), "tag");
    }

    #[test]
    fn invalid_tag_reports_tag_errors() {
        let e = tag_error("tag/path");
        assert_eq!(e.kind(), err::Kind::TagInvalidChar);
        assert_eq!(e.index(), 3); // the '/'
        assert_eq!("tag/path".as_bytes().get(e.index() as usize), Some(&b'/'));

        let e = tag_error("ta g");
        assert_eq!(e.kind(), err::Kind::TagInvalidChar);
        assert_eq!(e.index(), 2); // the ' '

        let e = tag_error("");
        assert_eq!(e.kind(), err::Kind::TagMissing);
        assert_eq!(e.index(), 0); // EOF
    }
}
