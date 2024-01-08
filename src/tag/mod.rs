use crate::{
    ambiguous::port_or_tag::{Kind as TagKind, OptionalPortOrTag},
    err::{Error, Kind as ErrorKind},
    span::{impl_span_methods_on_tuple, IntoOption, OptionalSpan, U},
};
#[derive(Clone, Copy)]
pub(crate) struct OptionalTagSpan<'src>(OptionalSpan<'src>);
impl_span_methods_on_tuple!(OptionalTagSpan);
impl<'src> IntoOption for OptionalTagSpan<'src> {
    fn is_some(&self) -> bool {
        self.short_len() > 0
    }
    fn none() -> Self
    where
        Self: Sized,
    {
        Self(OptionalSpan::new(0))
    }
}
impl<'src> TryFrom<OptionalPortOrTag<'src>> for OptionalTagSpan<'src> {
    type Error = Error;
    fn try_from(optional_port_or_tag: OptionalPortOrTag<'src>) -> Result<Self, Error> {
        match optional_port_or_tag.kind() {
            TagKind::Either | TagKind::Tag => Ok(if optional_port_or_tag.is_some() {
                Self(optional_port_or_tag.span())
            } else {
                Self::none()
            }),
            TagKind::Port => Err(Error(ErrorKind::TagInvalidChar, 0)), // FIXME: identify the invalid character
        }
    }
}
impl<'src> OptionalTagSpan<'src> {
    pub(crate) fn new(src: &str) -> Result<Self, Error> {
        let span = OptionalPortOrTag::new(src, TagKind::Tag).map_err(|e| {
            use crate::ambiguous::port_or_tag::Error as E;
            match e {
                E::TooLong(len) => Error(ErrorKind::TagTooLong, len),
                E::InvalidChar(len) => Error(ErrorKind::TagInvalidChar, len),
            }
        })?;
        debug_assert!(span.kind() == TagKind::Tag);
        Ok(Self(span.span()))
    }
}
