use crate::{
    ambiguous::host_or_path::{Error as HostOrPathError, Kind as HostKind, OptionalHostOrPath},
    err::Error,
    span::{impl_span_methods_on_tuple, IntoOption, OptionalSpan, U},
};
enum ErrorKind {
    NoMatch,
    ComponentInvalidEnd,
    InvalidChar,
    TooLong,

    Ipv6NoMatch,
    Ipv6TooLong,
    Ipv6BadColon,
    Ipv6TooManyHexDigits,
    Ipv6TooManyGroups,
    Ipv6TooFewGroups,
    Ipv6MissingClosingBracket,
}
pub(super) struct HostError(ErrorKind, U);
impl From<HostOrPathError> for HostError {
    fn from(err: HostOrPathError) -> Self {
        use crate::ambiguous::host_or_path::AmbiguousErrorKind as Src;
        use ErrorKind as Dest;
        match err.kind() {
            Src::NoMatch => HostError(Dest::NoMatch, err.len()),
            Src::TooLong => HostError(Dest::TooLong, err.len()),
            Src::InvalidChar => HostError(Dest::InvalidChar, err.len()),
            Src::Ipv6NoMatch => HostError(Dest::Ipv6NoMatch, err.len()),
            Src::Ipv6TooLong => HostError(Dest::Ipv6TooLong, err.len()),
            Src::Ipv6BadColon => HostError(Dest::Ipv6BadColon, err.len()),
            Src::Ipv6TooManyHexDigits => HostError(Dest::Ipv6TooManyHexDigits, err.len()),
            Src::Ipv6TooManyGroups => HostError(Dest::Ipv6TooManyGroups, err.len()),
            Src::Ipv6TooFewGroups => HostError(Dest::Ipv6TooFewGroups, err.len()),
            Src::Ipv6MissingClosingBracket => HostError(Dest::Ipv6MissingClosingBracket, err.len()),
        }
    }
}
impl From<HostError> for Error {
    fn from(err: HostError) -> Error {
        use crate::err::Kind as Dest;
        use ErrorKind as Src;
        let (kind, len) = (err.0, err.1);
        let kind = match kind {
            Src::NoMatch => Dest::HostNoMatch,
            Src::ComponentInvalidEnd => Dest::HostComponentInvalidEnd,
            Src::InvalidChar => Dest::HostInvalidChar,
            Src::TooLong => Dest::HostTooLong,
            Src::Ipv6NoMatch => Dest::Ipv6NoMatch,
            Src::Ipv6TooLong => Dest::Ipv6TooLong,
            Src::Ipv6BadColon => Dest::Ipv6BadColon,
            Src::Ipv6TooManyHexDigits => Dest::Ipv6TooManyHexDigits,
            Src::Ipv6TooManyGroups => Dest::Ipv6TooManyGroups,
            Src::Ipv6TooFewGroups => Dest::Ipv6TooFewGroups,
            Src::Ipv6MissingClosingBracket => Dest::Ipv6MissingClosingBracket,
        };
        Error(kind, len)
    }
}

use super::ipv6::Ipv6Span;

#[derive(Clone, Copy)]
pub(crate) enum Kind {
    /// a span of characters that represents a restricted domain name, e.g. "Example.com".
    /// TODO: note the restrictions
    Domain,
    /// an IPv6 address wrapped in square brackets, e.g. "[2001:db8::1]"
    Ipv6,
    /// Missing altogether
    Empty,
}

#[derive(Clone, Copy)]
pub(crate) struct OptionalHostSpan<'src>(pub(crate) OptionalSpan<'src>, pub(crate) Kind);
impl_span_methods_on_tuple!(OptionalHostSpan);
impl<'src> TryFrom<OptionalHostOrPath<'src>> for OptionalHostSpan<'_> {
    type Error = Error;
    fn try_from(ambiguous: OptionalHostOrPath) -> Result<Self, Error> {
        match ambiguous.into_option() {
            None => Ok(OptionalHostSpan::none()),
            Some(_) => {
                use HostKind::*;
                match ambiguous.kind() {
                    Either | Host => {
                        Ok(Self(OptionalSpan::new(ambiguous.short_len()), Kind::Domain))
                    }
                    IpV6 => Ok(Self(OptionalSpan::new(ambiguous.short_len()), Kind::Ipv6)),
                    Path => Err(Error(crate::err::Kind::HostInvalidChar, 0)), // <- needs the source str to find the index of the first underscore
                }
            }
        }
    }
}

impl<'src> TryFrom<&'src str> for OptionalHostSpan<'src> {
    type Error = Error;
    fn try_from(src: &'src str) -> Result<Self, Error> {
        OptionalHostOrPath::new(src, HostKind::Either)
            .map_err(|e| Into::<HostError>::into(e))?
            .try_into()
            .map_err(|e: Error| match e.0 {
                crate::err::Kind::HostInvalidChar => Error(
                    // this error only occurs if there was an underscore in the source str,
                    // it doesn't carry the location of the offending character.
                    // Here, we find the index of the first underscore using the source str.
                    crate::err::Kind::HostInvalidChar,
                    src.find('_').unwrap().try_into().unwrap(),
                ),
                _ => e,
            })
    }
}

impl<'src> OptionalHostSpan<'src> {
    pub(crate) fn kind(&self) -> Kind {
        self.1
    }
    pub(crate) fn new(src: &'src str) -> Result<Self, Error> {
        // handle bracketed ipv6 addresses
        OptionalHostOrPath::new(src, HostKind::Either)
            .map_err(|e| Into::<HostError>::into(e))?
            .try_into()
    }
}
impl<'src> From<Ipv6Span<'src>> for OptionalHostSpan<'src> {
    fn from(ipv6: Ipv6Span<'src>) -> Self {
        Self(ipv6.span().into(), Kind::Ipv6)
    }
}

impl<'src> IntoOption for OptionalHostSpan<'src> {
    fn is_some(&self) -> bool {
        self.short_len() > 0
    }
    fn none() -> Self
    where
        Self: Sized,
    {
        Self(OptionalSpan::new(0), Kind::Empty)
    }
}

pub(crate) struct HostStr<'src>(Kind, &'src str);
impl<'src> HostStr<'src> {
    fn src(&self) -> &'src str {
        self.1
    }
    pub(crate) fn kind(&self) -> Kind {
        self.0
    }
    #[inline(always)]
    fn len(&self) -> usize {
        self.src().len()
    }
    fn short_len(&self) -> U {
        self.len().try_into().unwrap() // this is safe since the length of a HostStr is always <= U::MAX
    }
    pub(super) fn from_span_of(
        src: &'src str,
        OptionalHostSpan(span, kind): OptionalHostSpan<'src>,
    ) -> Self {
        Self(kind, span.of(src))
    }
    pub fn from_prefix(src: &'src str) -> Result<Self, Error> {
        let span = OptionalHostSpan::new(src)?;
        Ok(HostStr::from_span_of(src, span))
    }
    pub fn from_exact_match(src: &'src str) -> Result<Self, Error> {
        let result = HostStr::from_prefix(src)?;
        if result.len() != src.len() {
            return Err(Error(crate::err::Kind::HostNoMatch, result.short_len()));
        }
        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    //
}
