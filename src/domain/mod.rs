//! > ```ebnf
//! > domain           := host (':' port-number)?
//! > host             := domain-name | IPv4address | ('[' IPv6address ']') ; rfc3986 appendix-A
//! > domain-name      := domain-component ['.' domain-component]*
//! > domain-component := [a-zA-Z0-9] | [a-zA-Z0-9][a-zA-Z0-9-]*[a-zA-Z0-9])
//! > port-number      := [0-9]+
//! > ```
//! Note that this **DOES NOT** allow for percent-encoded domain names. Thus,
//! we can't use the `url` crate for parsing domain names.
//! Since `url::Host` needs to decode percent-encoded domain names per [rfc3986](https://www.rfc-editor.org/rfc/rfc3986#appendix-A)

// use core::net::AddrParseError;
pub(crate) mod host;
pub(crate) mod ipv6;
pub(crate) mod port;
use crate::{
    ambiguous::domain_or_tagged_ref::{
        DomainOrRef, ErrKind as AmbiguousErrorKind, Error as AmbiguousError,
    },
    domain::{
        host::{HostStr, OptionalHostSpan},
        port::OptionalPortSpan,
    },
    err::{self, Error, Kind as ErrorKind},
    span::{IntoOption, SpanMethods, U},
};

/// a definite host and an optional port
#[derive(Clone, Copy)]
pub(super) struct DomainSpan<'src> {
    host: OptionalHostSpan<'src>,          // cannot be zero-length
    optional_port: OptionalPortSpan<'src>, // can be 0-length, indicating missing
}

/// a possibly missing host which may or may not have a port
#[derive(Clone, Copy)]
pub(crate) struct OptionalDomainSpan<'src>(DomainSpan<'src>);
impl<'src> IntoOption for OptionalDomainSpan<'src> {
    fn is_some(&self) -> bool {
        self.0.short_len() > 0
    }
    fn none() -> Self {
        Self(DomainSpan {
            host: OptionalHostSpan::none(),
            optional_port: OptionalPortSpan::none(),
        })
    }
}
impl<'src> OptionalDomainSpan<'src> {
    pub(crate) fn from_domain(domain: DomainSpan<'src>) -> Self {
        Self(domain)
    }
    #[inline(always)]
    pub(crate) fn len(&self) -> usize {
        self.0.len()
    }
    #[inline(always)]
    pub(crate) fn short_len(&self) -> U {
        self.0.short_len()
    }
    pub(crate) fn from_parts(
        host: OptionalHostSpan<'src>,
        optional_port: OptionalPortSpan<'src>,
    ) -> Self {
        Self(DomainSpan {
            host,
            optional_port,
        })
    }
}

impl<'src> TryFrom<DomainOrRef<'src>> for OptionalDomainSpan<'src> {
    type Error = Error;
    fn try_from(ambiguous: DomainOrRef<'src>) -> Result<Self, Error> {
        Ok(Self(DomainSpan {
            host: ambiguous.host_or_path.try_into()?,
            optional_port: ambiguous.optional_port_or_tag.try_into()?,
        }))
    }
}

fn disambiguate_error(e: AmbiguousError) -> Error {
    match e.kind() {
        AmbiguousErrorKind::LeftNoMatch => Error(err::Kind::HostNoMatch, e.len()),
        AmbiguousErrorKind::LeftInvalidChar => Error(err::Kind::HostInvalidChar, e.len()),
        AmbiguousErrorKind::LeftTooLong => Error(err::Kind::HostTooLong, e.len()),
        AmbiguousErrorKind::RightInvalidChar => Error(err::Kind::PortInvalidChar, e.len()),
        AmbiguousErrorKind::RightTooLong => Error(err::Kind::PortTooLong, e.len()),
        AmbiguousErrorKind::Ipv6NoMatch => Error(err::Kind::Ipv6NoMatch, e.len()),
        AmbiguousErrorKind::Ipv6TooLong => Error(err::Kind::Ipv6TooLong, e.len()),
        AmbiguousErrorKind::Ipv6BadColon => Error(err::Kind::Ipv6BadColon, e.len()),
        AmbiguousErrorKind::Ipv6TooManyHexDigits => Error(err::Kind::Ipv6TooManyHexDigits, e.len()),
        AmbiguousErrorKind::Ipv6TooManyGroups => Error(err::Kind::Ipv6TooManyGroups, e.len()),
        AmbiguousErrorKind::Ipv6TooFewGroups => Error(err::Kind::Ipv6TooFewGroups, e.len()),
        AmbiguousErrorKind::Ipv6MissingClosingBracket => {
            Error(err::Kind::Ipv6MissingClosingBracket, e.len())
        }
    }
}
impl<'src> TryFrom<Result<DomainOrRef<'src>, AmbiguousError>> for OptionalDomainSpan<'src> {
    type Error = Error;

    fn try_from(value: Result<DomainOrRef<'src>, AmbiguousError>) -> Result<Self, Self::Error> {
        value.map_err(disambiguate_error)?.try_into()
    }
}

impl<'src> DomainSpan<'src> {
    #[inline(always)]
    pub(super) fn len(&self) -> usize {
        self.host.len() + self.optional_port.len()
    }
    #[inline(always)]
    pub(super) fn short_len(&self) -> U {
        self.host.short_len() + self.optional_port.short_len()
    }
    pub(crate) fn host(&self) -> OptionalHostSpan<'src> {
        self.host
    }

    pub(crate) fn new(src: &'src str) -> Result<Self, Error> {
        let host = OptionalHostSpan::try_from(src)?;
        let optional_port = OptionalPortSpan::try_from(&src[host.len()..])?;
        Ok(Self {
            host,
            optional_port,
        })
    }
    pub fn port(&self) -> OptionalPortSpan {
        self.optional_port
    }
}

impl<'src> From<DomainSpan<'src>> for OptionalDomainSpan<'src> {
    fn from(domain: DomainSpan<'src>) -> Self {
        Self::from_domain(domain)
    }
}
pub struct DomainStr<'src> {
    pub src: &'src str,
    /// the host part of the domain. It can be an IPv4 address, an IPv6 address,
    /// or a restricted, non-percent-encoded domain name.
    span: DomainSpan<'src>,
}
impl<'src> DomainStr<'src> {
    #[inline(always)]
    pub fn len(&self) -> usize {
        self.src.len()
    }
    pub fn from_prefix(src: &'src str) -> Result<Self, Error> {
        let span = DomainSpan::new(src)?;
        Ok(Self {
            src: &src[..span.len()],
            span,
        })
    }
    pub fn from_exact_match(src: &'src str) -> Result<Self, Error> {
        let result = DomainStr::from_prefix(src)?;
        if result.len() != src.len() {
            // TODO: better error type?
            return Err(Error(ErrorKind::HostNoMatch, result.span.short_len()));
        }
        Ok(result)
    }
    pub fn host(&self) -> HostStr<'src> {
        HostStr::from_span_of(self.src, self.span.host)
    }
    pub fn port(&self) -> Option<&str> {
        self.span.optional_port.into_option().map(|port| {
            let start = self.span.host.len();
            &self.src[start..start + port.len()]
        })
    }
}
