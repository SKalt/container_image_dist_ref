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
    ambiguous::{host_or_path::HostOrPathSpan, port_or_tag::PortOrTagSpan},
    domain::{
        host::{HostSpan, HostStr},
        port::PortSpan,
    },
    err::{self, Error, Kind as ErrorKind},
    span::{IntoOption, Lengthy, Short},
};

/// a definite host and an optional port
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) struct DomainSpan<'src> {
    /// required: cannot be zero-length
    host: HostSpan<'src>,
    /// can be 0-length, indicating that the port is missing
    port: PortSpan<'src>,
}

impl Lengthy<'_, Short> for DomainSpan<'_> {
    #[inline(always)]
    fn short_len(&self) -> Short {
        self.host().short_len() + self.port().short_len()
    }
}

impl<'src> IntoOption for DomainSpan<'src> {
    fn is_some(&self) -> bool {
        self.short_len() > 0
    }
    fn none() -> Self {
        Self {
            host: HostSpan::none(),
            port: PortSpan::none(),
        }
    }
}

impl<'src> DomainSpan<'src> {
    pub(crate) fn host(&self) -> HostSpan<'src> {
        self.host
    }

    pub fn port(&self) -> PortSpan {
        self.port
    }

    pub(crate) fn new(src: &'src str) -> Result<Self, Error> {
        let host = HostSpan::try_from(src)?;
        let port = PortSpan::new(&src[host.len()..]).map_err(|e| e + host.short_len())?;
        Ok(Self { host, port })
    }

    pub(crate) fn from_ambiguous_parts(
        host: HostOrPathSpan<'src>,
        port: PortOrTagSpan<'src>,
        context: &'src str,
    ) -> Result<Self, Error> {
        debug_assert!(
            host.len() + port.len() <= context.len(),
            "ambiguous.len() = {}, context.len() = {}, context = {}",
            host.len() + port.len(),
            context.len(),
            context
        );

        let host = HostSpan::from_ambiguous(host, context)?;
        if host.is_none() {
            return Err(Error(0, err::Kind::HostNoMatch));
        }
        let port = PortSpan::from_ambiguous(port, &context[host.len()..])?;
        Ok(Self { host, port })
    }
}

pub struct DomainStr<'src> {
    src: &'src str,
    /// the host part of the domain. It can be an IPv4 address, an IPv6 address,
    /// or a restricted, non-percent-encoded domain name.
    span: DomainSpan<'src>,
}
impl<'src> DomainStr<'src> {
    pub fn src(&self) -> &'src str {
        self.src
    }
    pub fn len(&self) -> usize {
        self.src.len()
    }
    pub fn is_empty(&self) -> bool {
        self.src.is_empty()
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
            return Err(Error(result.span.short_len(), ErrorKind::HostNoMatch));
        }
        Ok(result)
    }
    pub fn host(&self) -> HostStr<'src> {
        HostStr::from_span_of(self.src, self.span.host)
    }
    pub fn port(&self) -> Option<&str> {
        self.span.port.into_option().map(|port| {
            let start = self.span.host.len();
            &self.src[start..start + port.len()]
        })
    }
}
