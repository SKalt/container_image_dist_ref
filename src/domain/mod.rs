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

pub(crate) mod host;
pub(crate) mod ipv6;
pub(crate) mod port;
use crate::{
    ambiguous::{host_or_path::OptionalHostOrPath, port_or_tag::PortOrTag},
    domain::{
        host::{HostStr, OptionalHostSpan},
        port::OptionalPortSpan,
    },
    err::{self, Error, Kind as ErrorKind},
    span::{IntoOption, Lengthy, Short},
};

/// a definite host and an optional port
#[derive(Clone, Copy)]
pub(super) struct OptionalDomainSpan<'src> {
    host: OptionalHostSpan<'src>,          // cannot be zero-length
    optional_port: OptionalPortSpan<'src>, // can be 0-length, indicating missing
}

impl Lengthy<'_, Short> for OptionalDomainSpan<'_> {
    #[inline(always)]
    fn short_len(&self) -> Short {
        self.host().short_len() + self.port().short_len()
    }
}

impl<'src> IntoOption for OptionalDomainSpan<'src> {
    fn is_some(&self) -> bool {
        self.short_len() > 0
    }
    fn none() -> Self {
        Self {
            host: OptionalHostSpan::none(),
            optional_port: OptionalPortSpan::none(),
        }
    }
}

impl<'src> OptionalDomainSpan<'src> {
    pub(crate) fn host(&self) -> OptionalHostSpan<'src> {
        self.host
    }

    pub fn port(&self) -> OptionalPortSpan {
        self.optional_port
    }

    pub(crate) fn new(src: &'src str) -> Result<Self, Error> {
        let host = OptionalHostSpan::try_from(src)?;
        let optional_port =
            OptionalPortSpan::new(&src[host.len()..]).map_err(|e| e + host.short_len())?;
        Ok(Self {
            host,
            optional_port,
        })
    }
    pub(crate) fn from_parts(
        host: OptionalHostSpan<'src>,
        optional_port: OptionalPortSpan<'src>,
    ) -> Self {
        Self {
            host,
            optional_port,
        }
    }

    // pub(crate) fn from_ambiguous(
    //     ambiguous: OptionalHostOrPath<'src>,
    //     context: &'src str,
    // ) -> Result<Self, Error> {
    //     let host = OptionalHostSpan::try_from(ambiguous).map_err(disambiguate_error)?;
    //     let optional_port =
    //         OptionalPortSpan::new(&context[host.len()..]).map_err(|e| e + host.short_len())?;
    //     Ok(Self {
    //         host,
    //         optional_port,
    //     })
    // }
    pub(crate) fn from_ambiguous_parts(
        host: OptionalHostOrPath<'src>,
        optional_port: PortOrTag<'src>,
    ) -> Result<Self, Error> {
        let host = OptionalHostSpan::from_ambiguous(host)?;
        if host.is_none() {
            return Error::at(0, err::Kind::HostNoMatch);
        }
        let optional_port = OptionalPortSpan::from_ambiguous(optional_port)?;
        Ok(Self {
            host,
            optional_port,
        })
    }
}

pub struct DomainStr<'src> {
    pub src: &'src str,
    /// the host part of the domain. It can be an IPv4 address, an IPv6 address,
    /// or a restricted, non-percent-encoded domain name.
    span: OptionalDomainSpan<'src>,
}
impl<'src> DomainStr<'src> {
    #[inline(always)]
    pub fn len(&self) -> usize {
        self.src.len()
    }
    pub fn from_prefix(src: &'src str) -> Result<Self, Error> {
        let span = OptionalDomainSpan::new(src)?;
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
