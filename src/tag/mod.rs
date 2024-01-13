use crate::{
    ambiguous::port_or_tag::{Kind as TagKind, PortOrTag},
    err::{self, Error},
    span::{impl_span_methods_on_tuple, IntoOption, Lengthy, ShortLength},
};
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) struct TagSpan<'src>(ShortLength<'src>);
impl_span_methods_on_tuple!(TagSpan, Short);
impl<'src> IntoOption for TagSpan<'src> {
    fn is_some(&self) -> bool {
        self.short_len() > 0
    }
    fn none() -> Self {
        Self(0.into())
    }
}

impl<'src> From<PortOrTag<'src>> for TagSpan<'src> {
    fn from(optional_port_or_tag: PortOrTag<'src>) -> Self {
        Self(optional_port_or_tag.span())
    }
}
fn disambiguate_error(e: Error) -> Error {
    let kind = match e.kind() {
        err::Kind::PortOrTagTooLong => err::Kind::TagTooLong,
        err::Kind::PortOrTagInvalidChar => err::Kind::TagInvalidChar,
        _ => e.kind(),
    };
    Error(e.index(), kind)
}
impl<'src> TagSpan<'src> {
    /// can match an empty span if the first character in `src` is a `/` or `@`
    pub(crate) fn new(src: &str) -> Result<Self, Error> {
        let span = PortOrTag::new(src, TagKind::Tag).map_err(disambiguate_error)?;
        debug_assert!(span.kind() == TagKind::Tag);
        Ok(Self(span.span()))
    }
    // pub(crate) fn from_ambiguous(
    //     ambiguous: OptionalPortOrTag<'src>,
    //     context: &'src str,
    // ) -> Result<Self, Error> {
    //     match ambiguous.kind() {
    //         TagKind::Either | TagKind::Tag => Ok(Self(ambiguous.into_span())),
    //         TagKind::Port => Err(Error(
    //             err::Kind::TagInvalidChar,
    //             ambiguous.span_of(context)
    //                 .bytes()
    //                 .find(|b| !b.is_ascii_alphanumeric())
    //                 .unwrap() // safe since ambiguous.kind == Port, which means there must be a non-alphanumeric char
    //                 .try_into()
    //                 .unwrap(), // safe since ambiguous.span_of(context) must be short
    //         )),
    //     }
    // }
}
