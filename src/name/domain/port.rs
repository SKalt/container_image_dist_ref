use core::num::NonZeroU8;

use crate::{
    ambiguous::port_or_tag::{Kind as PortKind, PortOrTagSpan},
    err,
    span::{impl_span_methods_on_tuple, Lengthy, ShortLength},
};

type Error = err::Error<u8>;

/// A span representing a port number **WITHOUT** the leading colon.
/// Max length = 255ch, enforced in [`PortOrTagSpan::new`]. When part of a
/// `DomainSpan`, the combined host/port length is limited to 255 characters.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) struct PortSpan<'src>(ShortLength<'src>);
impl_span_methods_on_tuple!(PortSpan, u8, NonZeroU8);

const fn disambiguate_err(e: Error) -> Error {
    let kind = match e.kind() {
        err::Kind::PortOrTagInvalidChar => err::Kind::PortInvalidChar,
        _ => e.kind(),
    };
    Error::at(e.index(), kind)
}

impl<'src> PortSpan<'src> {
    /// parse a port from the start of a string. Does NOT include the leading colon.
    pub(super) fn new(src: &'src str) -> Result<Self, Error> {
        let span = PortOrTagSpan::new(src, PortKind::Port).map_err(disambiguate_err)?;
        // A string that started like a port but contains non-digits must be a tag,
        // so it cannot be narrowed back to a port.
        span.narrow(PortKind::Port)
            .map(|span| Self(span.span()))
            .map_err(disambiguate_err)
    }
}

impl<'src> TryFrom<PortOrTagSpan<'src>> for PortSpan<'src> {
    type Error = Error;
    fn try_from(ambiguous: PortOrTagSpan<'src>) -> Result<Self, Error> {
        ambiguous
            .narrow(PortKind::Port)
            .map(|span| Self(span.span()))
    }
}
