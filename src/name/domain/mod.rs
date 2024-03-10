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
use core::num::NonZeroU16;

use crate::{
    ambiguous::{host_or_path::HostOrPathSpan, port_or_tag::PortOrTagSpan},
    domain::{
        host::{HostSpan, HostStr},
        port::PortSpan,
    },
    err::{self, Kind as ErrorKind},
    span::{Lengthy, OptionallyZero},
};
type Error = err::Error<u16>;

/// a definite host and an optional port. Combined length MUST be under 255 chars.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) struct DomainSpan<'src> {
    /// The length of the source string that represents the host
    pub host: HostSpan<'src>,
    /// If present, the length of the source string that represents the port *after* a colon
    pub port: Option<PortSpan<'src>>,
}

impl Lengthy<'_, u16, NonZeroU16> for DomainSpan<'_> {
    #[inline]
    fn short_len(&self) -> NonZeroU16 {
        let mut len = self.host.short_len().widen(); // since host can be up to 255 chars, pad to avoid overflow
        len = len
            .checked_add(self
                .port.map(|p| p.short_len().upcast() + 1) // +1 for the leading ':'
                .unwrap_or(0) as u16)
            .unwrap();
        len
    }
    #[inline]
    fn len(&self) -> usize {
        self.short_len().as_usize()
    }
}

/// constructor methods
impl<'src> DomainSpan<'src> {
    /// check that a given HostSpan and PortSpan can be combined into a DomainSpan
    /// without overflowing the 255 char limit
    fn from_parts(host: HostSpan<'src>, port: Option<PortSpan<'src>>) -> Result<Self, Error> {
        if let Some(port) = port {
            host.short_len()
                .checked_add(port.short_len().upcast())
                .ok_or(Error::at(u8::MAX.into(), err::Kind::PortTooLong))?;
        }
        Ok(Self { host, port })
    }
    /// parse a domain from the start of a string.
    pub(crate) fn new(src: &'src str) -> Result<Option<Self>, Error> {
        if let Some(host) = HostSpan::new(src)? {
            let port = PortSpan::new(&src[host.len()..])
                .map_err(|e| e.into())
                .map_err(|e: Error| e + host.short_len().widen())?;
            Self::from_parts(host, port).map(Some)
        } else {
            Ok(None)
        }
    }

    pub(crate) fn from_ambiguous(
        host: HostOrPathSpan<'src>,
        port: Option<PortOrTagSpan<'src>>,
    ) -> Result<Self, Error> {
        let host = HostSpan::from_ambiguous(host)?;
        let port = if let Some(p) = port {
            Some(PortSpan::from_ambiguous(p)?)
        } else {
            None
        };
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
    pub fn to_str(&self) -> &'src str {
        self.src
    }
    pub fn len(&self) -> usize {
        self.src.len()
    }
    pub fn is_empty(&self) -> bool {
        self.src.is_empty()
    }
    #[inline]
    pub(crate) fn from_span(span: DomainSpan<'src>, src: &'src str) -> Self {
        debug_assert_eq!(span.len(), src.len(), "{src:?}.len() != {}", span.len());
        Self { src, span }
    }
    pub fn from_prefix(src: &'src str) -> Result<Option<Self>, Error> {
        Ok(DomainSpan::new(src)?.map(|span| Self { src, span }))
    }
    pub fn from_exact_match(src: &'src str) -> Result<Option<Self>, Error> {
        let result = DomainSpan::new(src)?;
        let len = result.as_ref().map(|r| r.short_len().into()).unwrap_or(0);
        if (len as usize) != src.len() {
            return Err(Error::at(len, ErrorKind::HostInvalidChar));
        }
        Ok(result.map(|span| Self { src, span }))
    }
    pub fn host(&self) -> HostStr<'src> {
        HostStr::from_span_of(self.src, self.span.host)
    }
    pub fn port(&self) -> Option<&str> {
        self.span.port.map(|port| {
            let start = self.span.host.len();
            &self.src[start..start + port.len()]
        })
    }
}
