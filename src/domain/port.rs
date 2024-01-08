use crate::{
    ambiguous::port_or_tag::{Kind as PortKind, OptionalPortOrTag},
    err::{Error, Kind as ErrorKind},
    span::{impl_span_methods_on_tuple, IntoOption, OptionalSpan, U},
};

// pub(crate) use crate::ambiguous::port_or_tag::Error;
/// a span representing a port number **WITH** the leading colon. Can be empty.
#[derive(Clone, Copy)]
pub(crate) struct OptionalPortSpan<'src>(OptionalSpan<'src>);
impl_span_methods_on_tuple!(OptionalPortSpan);

fn convert_err(e: crate::ambiguous::port_or_tag::Error) -> Error {
    use crate::ambiguous::port_or_tag::Error as E;
    match e {
        E::TooLong(len) => Error(ErrorKind::PortTooLong, len),
        E::InvalidChar(len) => Error(ErrorKind::PortInvalidChar, len),
    }
}
impl<'src> IntoOption for OptionalPortSpan<'src> {
    fn is_some(&self) -> bool {
        self.short_len() > 0
    }
    fn none() -> Self {
        Self(OptionalSpan::new(0))
    }
}
impl<'src> TryFrom<OptionalPortOrTag<'src>> for OptionalPortSpan<'src> {
    type Error = Error;
    fn try_from(optional_port_or_tag: OptionalPortOrTag<'src>) -> Result<Self, Error> {
        match optional_port_or_tag.kind() {
            PortKind::Either | PortKind::Port => Ok(if optional_port_or_tag.is_some() {
                Self(optional_port_or_tag.span())
            } else {
                Self::none()
            }),
            PortKind::Tag => Err(Error(ErrorKind::PortInvalidChar, 0)), // FIXME: identify the invalid character
        }
    }
}
impl<'src> TryFrom<&'src str> for OptionalPortSpan<'src> {
    type Error = Error;
    fn try_from(src: &'src str) -> Result<Self, Error> {
        OptionalPortOrTag::new(src, PortKind::Either)
            .map_err(convert_err)?
            .try_into()
    }
}

impl OptionalPortSpan<'_> {
    pub(crate) fn none() -> Self {
        Self(OptionalSpan::new(0))
    }
    pub(super) fn new(src: &str) -> Result<Self, Error> {
        OptionalPortOrTag::new(src, PortKind::Port)
            .map_err(convert_err)?
            .try_into()
    }
}
