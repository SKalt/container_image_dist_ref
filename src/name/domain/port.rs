use core::num::NonZeroU8;

use crate::{
    ambiguous::port_or_tag::{Kind as PortKind, PortOrTagSpan},
    err,
    span::{impl_span_methods_on_tuple, Lengthy, ShortLength},
};

type Error = err::Error<u8>;

// pub(crate) use crate::ambiguous::port_or_tag::Error;
/// a span representing a port number **WITH** the leading colon. Can be empty.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) struct PortSpan<'src>(ShortLength<'src>);
impl_span_methods_on_tuple!(PortSpan, u8, NonZeroU8);

fn disambiguate_err(e: Error) -> Error {
    let kind = match e.kind() {
        err::Kind::PortOrTagInvalidChar => err::Kind::PortInvalidChar,
        _ => e.kind(),
    };
    Error::at(e.index(), kind)
}

impl<'src> PortSpan<'src> {
    pub(super) fn new(src: &'src str) -> Result<Option<Self>, Error> {
        Ok(PortOrTagSpan::new(src, PortKind::Port)
            .map_err(disambiguate_err)?
            .map(|span| Self(span.span())))
    }
    pub(super) fn from_ambiguous(ambiguous: PortOrTagSpan<'src>) -> Result<Self, Error> {
        ambiguous
            .narrow(PortKind::Port)
            .map(|span| Self(span.span()))
    }
}
