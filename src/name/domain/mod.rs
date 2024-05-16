//! # Domain: host and an optional port
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
pub use host::{Host, Kind};

use crate::{
    ambiguous::{host_or_path::HostOrPathSpan, port_or_tag::PortOrTagSpan},
    domain::{host::HostSpan, port::PortSpan},
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
        self.host.short_len().widen()// since host can be up to 255 chars, pad to avoid overflow
            .saturating_add(
                self.port
                    .map(|p: PortSpan| p.short_len().upcast().saturating_add(1).into()) // +1 for the leading ':'
                    // safe since port is at most 128 chars
                    .unwrap_or(0u16),
        )
    }
    #[inline]
    fn len(&self) -> usize {
        self.short_len().as_usize()
    }
}

/// constructor methods
impl<'src> DomainSpan<'src> {
    /// check that a given `HostSpan` and `PortSpan` can be combined into a `DomainSpan`
    /// without overflowing the 255 char limit
    fn from_parts(host: HostSpan<'src>, port: Option<PortSpan<'src>>) -> Result<Self, Error> {
        if let Some(port) = port {
            host.short_len()
                .checked_add(port.short_len().upcast())
                .ok_or(Error::at(u8::MAX.into(), err::Kind::PortTooLong))?;
        }
        Ok(Self { host, port })
    }
    /// parse a domain from the start of a string. Can consume only part of the source
    /// string if it reaches a valid stopping point, i.e. `/` or `@`
    pub(crate) fn new(src: &'src str) -> Result<Self, Error> {
        let host = HostSpan::new(src)?;
        let len: u16 = host.short_len().widen().into(); // max 255 chars
        let port = match &src[host.len()..].bytes().next() {
            Some(b':') => PortSpan::new(&src[host.len() + 1..])
                .map(Some)
                .map_err(|e| Error::at(len.saturating_add(e.index().into()), e.kind())),
            Some(b'/' | b'@') | None => Ok(None),
            _ => Error::at(len, err::Kind::HostInvalidChar).into(),
        }?; // FIXME: checked add
        Self::from_parts(host, port)
    }

    pub(crate) fn from_ambiguous(
        host: HostOrPathSpan<'src>,
        port: Option<PortOrTagSpan<'src>>,
    ) -> Result<Self, Error> {
        let host = HostSpan::try_from(host)?;
        // FIXME: peek at next char
        let port = if let Some(p) = port {
            Some(PortSpan::try_from(p)?)
        } else {
            None
        };
        Self::from_parts(host, port)
    }
}

/// The domain component of an image reference is composed of a host name or ip
/// literal and an optional port number.
/// ```rust
/// use container_image_dist_ref::name::domain::Domain;
/// let domain = Domain::new("localhost:5000").unwrap();
/// assert_eq!(domain.host().to_str(), "localhost");
/// assert_eq!(domain.port(), Some("5000"));
/// ```
pub struct Domain<'src> {
    src: &'src str,
    /// the host part of the domain. It can be an IPv4 address, an IPv6 address,
    /// or a restricted, non-percent-encoded domain name.
    span: DomainSpan<'src>,
}
#[allow(clippy::len_without_is_empty)]
impl<'src> Domain<'src> {
    #[allow(missing_docs)]
    pub const fn to_str(&self) -> &'src str {
        self.src
    }
    #[allow(missing_docs)]
    pub const fn len(&self) -> usize {
        self.src.len()
    }
    #[inline]
    pub(crate) fn from_span(span: DomainSpan<'src>, src: &'src str) -> Self {
        debug_assert_eq!(span.len(), src.len(), "{src:?}.len() != {}", span.len());
        Self { src, span }
    }
    /// parse a domain from the start of a string. Can consume only part of the source
    /// string if it reaches a valid stopping point, i.e. `/` or `@`
    pub fn new(src: &'src str) -> Result<Self, Error> {
        let span = DomainSpan::new(src)?;
        Ok(Self::from_span(span, &src[..span.len()]))
    }
    /// checks that the entire string is parsed
    pub fn from_exact_match(src: &'src str) -> Result<Self, Error> {
        let result = Self::new(src)?;
        if result.len() != src.len() {
            return Err(Error::at(
                result.span.short_len().into(),
                ErrorKind::HostInvalidChar,
            ));
        }
        Ok(result)
    }
    #[allow(missing_docs)]
    pub fn host(&self) -> Host<'src> {
        Host::from_span(&self.src[0..self.span.host.len()], self.span.host)
    }
    /// Not including any leading `:`.
    pub fn port(&self) -> Option<&str> {
        let port = self.span.port?;
        let start = self.span.host.len() + 1; // +1 for the leading ':'
        Some(&self.src[start..start + port.len()])
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    #[test]
    fn temp() {
        Domain::new("localhost:5000").unwrap();
    }
}
