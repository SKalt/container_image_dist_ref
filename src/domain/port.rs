use crate::{
    ambiguous::port_or_tag::{Kind as PortKind, PortOrTagSpan},
    err::{self, Error},
    span::{impl_span_methods_on_tuple, IntoOption, Length, Lengthy},
};

// pub(crate) use crate::ambiguous::port_or_tag::Error;
/// a span representing a port number **WITH** the leading colon. Can be empty.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) struct PortSpan<'src>(Length<'src>);
impl_span_methods_on_tuple!(PortSpan, Short);

fn disambiguate_err(e: Error) -> Error {
    let kind = match e.kind() {
        err::Kind::PortOrTagTooLong => err::Kind::PortTooLong,
        err::Kind::PortOrTagInvalidChar => err::Kind::PortInvalidChar,
        _ => e.kind(),
    };
    Error::at(e.index(), kind)
}
impl<'src> IntoOption for PortSpan<'src> {
    fn is_some(&self) -> bool {
        self.short_len() > 0
    }
    fn none() -> Self {
        Self(Length::new(0))
    }
}

impl<'src> PortSpan<'src> {
    pub(super) fn new(src: &'src str) -> Result<Self, Error> {
        let span = PortOrTagSpan::new(src, PortKind::Port).map_err(disambiguate_err)?;
        match span.into_option() {
            None => Ok(Self::none()),
            Some(_) => Ok(Self(span.span())),
        }
    }
    pub(super) fn from_ambiguous(ambiguous: PortOrTagSpan<'src>) -> Result<Self, Error> {
        ambiguous
            .narrow(PortKind::Port)
            .map(|span| Self(span.span()))
    }
}
