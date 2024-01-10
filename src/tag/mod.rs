use crate::{
    ambiguous::port_or_tag::{Kind as TagKind, OptionalPortOrTag},
    err::{self, Error},
    span::{impl_span_methods_on_tuple, IntoOption, OptionalSpan, U},
};
#[derive(Clone, Copy)]
pub(crate) struct OptionalTagSpan<'src>(OptionalSpan<'src>);
impl_span_methods_on_tuple!(OptionalTagSpan);
impl<'src> IntoOption for OptionalTagSpan<'src> {
    fn is_some(&self) -> bool {
        self.short_len() > 0
    }
    fn none() -> Self {
        Self(OptionalSpan::new(0))
    }
}

impl<'src> From<OptionalPortOrTag<'src>> for OptionalTagSpan<'src> {
    fn from(optional_port_or_tag: OptionalPortOrTag<'src>) -> Self {
        Self(optional_port_or_tag.span())
    }
}
fn disambiguate_error(e: Error) -> Error {
    let kind = match e.kind() {
        err::Kind::PortOrTagTooLong => err::Kind::TagTooLong,
        err::Kind::PortOrTagInvalidChar => err::Kind::TagInvalidChar,
        _ => e.kind(),
    };
    Error(kind, e.index())
}
impl<'src> OptionalTagSpan<'src> {
    pub(crate) fn new(src: &str) -> Result<Self, Error> {
        let span = OptionalPortOrTag::new(src, TagKind::Tag).map_err(disambiguate_error)?;
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
