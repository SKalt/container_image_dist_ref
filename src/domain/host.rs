// > name                            := [domain '/'] remote-name
// > domain                          := host [':' port-number]
// > port-number                     := /[0-9]+/
// > host                            := domain-name | IPv4address | \[ IPv6address \] ; rfc3986 appendix-A
// > domain-name                     := domain-component ['.' domain-component]*
// > domain-component                := alpha-numeric [ ( alpha-numeric | '-' )* alpha-numeric ]
// > path-component                  := alpha-numeric [separator alpha-numeric]*
// > path (or "remote-name")         := path-component ['/' path-component]*
// > alpha-numeric                   := /[a-z0-9]+/
// > separator                       := /[_.]|__|[-]*/
// >
// > tag                             := /[\w][\w.-]{0,127}/
//
// Note that domain components conflict with path components:
// | class | domain-component | path-component |
// | ----- | ---------------- | -------------- |
// | upper | yes              | no             |
// | -     | inner            | inner          |
// | _     | no               | inner          |
// | .     | yes              | yes            |

// use crate::;
use crate::{
    ambiguous::host_or_path::{Error as HostOrPathError, Kind as HostKind, OptionalHostOrPath},
    err::{Error, Kind as ErrorKind},
    span::{impl_span_methods_on_tuple, IntoOption, OptionalSpan, U},
};
enum EKind {
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
pub(super) struct HostError(EKind, U);
impl From<HostOrPathError> for HostError {
    fn from(err: HostOrPathError) -> Self {
        use crate::ambiguous::host_or_path::AmbiguousErrorKind as A; // FIXME: rename
        match err.kind() {
            A::NoMatch => HostError(EKind::NoMatch, err.len()),
            A::TooLong => HostError(EKind::TooLong, err.len()),
            A::InvalidChar => HostError(EKind::InvalidChar, err.len()),
            A::Ipv6NoMatch => HostError(EKind::Ipv6NoMatch, err.len()),
            A::Ipv6TooLong => HostError(EKind::Ipv6TooLong, err.len()),
            A::Ipv6BadColon => HostError(EKind::Ipv6BadColon, err.len()),
            A::Ipv6TooManyHexDigits => HostError(EKind::Ipv6TooManyHexDigits, err.len()),
            A::Ipv6TooManyGroups => HostError(EKind::Ipv6TooManyGroups, err.len()),
            A::Ipv6TooFewGroups => HostError(EKind::Ipv6TooFewGroups, err.len()),
            A::Ipv6MissingClosingBracket => HostError(EKind::Ipv6MissingClosingBracket, err.len()),
        }
    }
}
impl From<HostError> for Error {
    fn from(err: HostError) -> Error {
        use EKind as Src;
        use ErrorKind as Dest;
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
            Some(a) => {
                use HostKind::*;
                match ambiguous.kind() {
                    Either | Host => {
                        Ok(Self(OptionalSpan::new(ambiguous.short_len()), Kind::Domain))
                    }
                    Path => Err(Error(ErrorKind::HostInvalidChar, 0)), // FIXME: find the underscore(s) in the path
                    IpV6 => Ok(Self(OptionalSpan::new(ambiguous.short_len()), Kind::Ipv6)),
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
            return Err(Error(
                ErrorKind::HostNoMatch,
                result.len().try_into().unwrap(),
            ));
            // FIXME: avoid panic
        }
        Ok(result)
    }
}
