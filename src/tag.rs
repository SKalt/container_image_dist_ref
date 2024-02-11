/*!
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

pub const TAG_MAX_LEN: NonZeroU8 = nonzero!(u8, 128);

// we can index all errors with a u8 since the longest possible tag is 128 characters
type Error = err::Error<u8>;

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) struct TagSpan<'src>(ShortLength<'src>);
impl_span_methods_on_tuple!(TagSpan, u8, NonZeroU8);

impl<'src> TryFrom<PortOrTagSpan<'src>> for TagSpan<'src> {
    type Error = Error;
    fn try_from(ambiguous: PortOrTagSpan<'src>) -> Result<Self, Error> {
        if ambiguous.short_len() <= TAG_MAX_LEN {
            Ok(Self(ambiguous.span()))
        } else {
            Err(Error::at(TAG_MAX_LEN.upcast(), err::Kind::TagTooLong))
        }
    }
}

impl<'src> TagSpan<'src> {
    /// can match an empty span if the first character in `src` is a `/` or `@`
    pub(crate) fn new(src: &str) -> Result<Option<Self>, Error> {
        Ok(PortOrTagSpan::new(src, TagKind::Tag)
            .map_err(|e| {
                let kind = match e.kind() {
                    err::Kind::PortOrTagInvalidChar => err::Kind::TagInvalidChar,
                    _ => e.kind(),
                };
                Error::at(e.index(), kind)
            })?
            .map(|span| Self(span.span())))
    }
}

pub struct TagStr<'src> {
    src: &'src str,
    span: TagSpan<'src>,
}
impl<'src> TagStr<'src> {
    pub fn new(src: &'src str) -> Result<Option<Self>, Error> {
        Ok(TagSpan::new(src)?.map(|span| Self { src, span }))
    }
    pub fn src(&self) -> &'src str {
        self.span.span_of(self.src)
    }
}
