use crate::{
    ambiguous::port_or_tag::{Kind as TagKind, PortOrTagSpan},
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

impl<'src> From<PortOrTagSpan<'src>> for TagSpan<'src> {
    fn from(ambiguous: PortOrTagSpan<'src>) -> Self {
        Self(ambiguous.span())
    }
}
fn disambiguate_error(e: Error) -> Error {
    let kind = match e.kind() {
        err::Kind::PortOrTagInvalidChar => err::Kind::TagInvalidChar,
        _ => e.kind(),
    };
    Error::at(e.index(), kind)
}
impl<'src> TagSpan<'src> {
    /// can match an empty span if the first character in `src` is a `/` or `@`
    pub(crate) fn new(src: &str) -> Result<Self, Error> {
        let span = PortOrTagSpan::new(src, TagKind::Tag).map_err(disambiguate_error)?;
        debug_assert!(span.kind() == TagKind::Tag);
        Ok(Self(span.span()))
    }
}
