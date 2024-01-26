//! # Domain
//! Parsers for the domain section of a reference:
//! ```txt
//! docker.io/library/alpine:3.14
//! ^^^^^^^^^
//! 127.0.0.1:5000/registry/alpine:3.14
//! ^^^^^^^^^^^^^^
//!  ```
//! The grammar for the domain section of a reference is:

// {{{sh
//    cat ../../grammars/reference.ebnf |
//      grep -E "^(domain|port|host)" |
//      sed 's#^#//! #g';
//    printf '//! ```\n\n// '
// }}}{{{out skip=2

//! ```ebnf
//! domain               ::= host (":" port-number)?
//! host                 ::= domain-name | IPv4address | "[" IPv6address "]" /* see https://www.rfc-editor.org/rfc/rfc3986#appendix-A */
//! domain-name          ::= domain-component ("." domain-component)*
//! domain-component     ::= ([a-zA-Z0-9]|[a-zA-Z0-9][a-zA-Z0-9-]*[a-zA-Z0-9])
//! port-number          ::= [0-9]+
//! ```

// }}}

//! Note that this **DOES NOT** allow for percent-encoded domain names. Thus,
//! we can't use the `url` crate for parsing domain names, since `url::Host`
//! strictly follows [RFC 3986](https://www.rfc-editor.org/rfc/rfc3986#appendix-A),
//! which allows decode percent-encoded domain names.

pub(crate) mod host;
pub(crate) mod ipv6;
pub(crate) mod port;
use crate::{
    ambiguous::{host_or_path::HostOrPathSpan, port_or_tag::PortOrTagSpan},
    domain::{
        host::{HostSpan, HostStr},
        port::PortSpan,
    },
    err::{self, Kind as ErrorKind},
    span::{IntoOption, Lengthy, Long, Short},
};
type Error = err::Error<Long>;

/// a definite host and an optional port. Combined length MUST be under 255 chars.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) struct DomainSpan<'src> {
    /// required: cannot be zero-length
    host: HostSpan<'src>,
    /// can be 0-length, indicating that the port is missing
    port: PortSpan<'src>,
}

impl Lengthy<'_, Long> for DomainSpan<'_> {
    #[inline(always)]
    fn short_len(&self) -> Long {
        self.host().short_len() as Long // since host can be up to 255 chars, pad to avoid overflow
            + self
                .port()
                .into_option()
                .map(|p| p.short_len() + 1) // +1 for the leading ':'
                .unwrap_or(0) as Long
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

/// accessor functions
impl<'src> DomainSpan<'src> {
    pub fn host(&self) -> HostSpan<'src> {
        self.host
    }

    pub fn port(&self) -> PortSpan {
        self.port
    }
}

/// constructor methods
impl<'src> DomainSpan<'src> {
    /// check that a given HostSpan and PortSpan can be combined into a DomainSpan
    /// without overflowing the 255 char limit
    fn from_parts(host: HostSpan<'src>, port: PortSpan<'src>) -> Result<Self, Error> {
        host.short_len()
            .checked_add(port.short_len())
            .ok_or(Error::at(Short::MAX.into(), err::Kind::PortTooLong))?;
        Ok(Self { host, port })
    }
    /// parse a domain from the start of a string.
    pub(crate) fn new(src: &'src str) -> Result<Self, Error> {
        let host = HostSpan::new(src)?;
        let port = PortSpan::new(&src[host.len()..]).map_err(|e| e + host.short_len())?;
        Self::from_parts(host, port)
    }

    pub(crate) fn from_ambiguous(
        host: HostOrPathSpan<'src>,
        port: PortOrTagSpan<'src>,
    ) -> Result<Self, Error> {
        let host = HostSpan::from_ambiguous(host)?;
        if host.is_none() {
            return Err(Error::at(0, err::Kind::HostNoMatch));
        }
        let port = PortSpan::from_ambiguous(port)?;
        Self::from_parts(host, port)
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
            return Err(Error::at(result.span.short_len(), ErrorKind::HostNoMatch));
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
