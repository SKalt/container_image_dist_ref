//! # Host
//! Parses something like
//! ```txt
//! docker.io/library/alpine:3.14
//! ^^^^^^^^^
//! localhost:5000/registry/alpine:3.14
//! ^^^^^^^^^^^^^^
//! [2001:db8::1]:5000/registry/alpine:3.14
//! ^^^^^^^^^^^^^^^^^
//! ```
//!
//! Specifically, the grammar is:

// {{{sh sed 's#^#//! #g' ../../grammars/host_subset.ebnf; printf '//! ```\n\n// ' }}}{{{out skip=2

//! ```ebnf
//! domain               ::= host (":" port-number)?
//! host                 ::= domain-name | IPv4address | "[" IPv6address "]" /* see https://www.rfc-editor.org/rfc/rfc3986#appendix-A */
//! domain-name          ::= domain-component ("." domain-component)*
//! domain-component     ::= ([a-zA-Z0-9]|[a-zA-Z0-9][a-zA-Z0-9-]*[a-zA-Z0-9])
//! port-number          ::= [0-9]+
//! ```

// }}}

use core::num::NonZeroU8;

use crate::{
    ambiguous::host_or_path::{HostOrPathSpan, Kind as HostKind},
    err,
    span::{impl_span_methods_on_tuple, nonzero, Length, Lengthy},
};

type Error = err::Error<u8>;

fn disambiguate_err(e: Error) -> Error {
    let kind = match e.kind() {
        err::Kind::HostOrPathInvalidChar => err::Kind::HostInvalidChar,
        err::Kind::HostOrPathTooLong => err::Kind::HostTooLong,
        err::Kind::HostOrPathMissing => err::Kind::HostMissing,
        _ => e.kind(),
    };
    Error::at(e.index(), kind)
}

use super::ipv6::Ipv6Span;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    /// a span of ascii characters that represents a restricted domain name, e.g. "Example.com".
    /// Must match the regex `^[a-zA-Z0-9][a-zA-Z0-9-]*[a-zA-Z0-9]$`
    Domain,
    /// a restricted IPv6 address wrapped in square brackets, e.g. `[2001:db8::1]`
    /// Unlike the IPv6 described in [RFC 3986](https://www.rfc-editor.org/rfc/rfc3986#appendix-A),
    /// IPv4 mapping is forbidden: only hex digits and colons are allowed.
    Ipv6,
    /// Missing altogether
    Empty,
}

/// can be ipv6
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) struct HostSpan<'src>(Length<'src, NonZeroU8>, Kind);
impl_span_methods_on_tuple!(HostSpan, u8, NonZeroU8);
impl<'src> TryFrom<HostOrPathSpan<'src>> for HostSpan<'_> {
    type Error = Error;
    fn try_from(ambiguous: HostOrPathSpan) -> Result<Self, Error> {
        use HostKind::*;

        match ambiguous.kind() {
            HostOrPath | Any | Host => Ok(Self(
                Length::from_nonzero(ambiguous.short_len()),
                Kind::Domain,
            )),
            IpV6 => Ok(Self(
                Length::from_nonzero(ambiguous.short_len()),
                Kind::Ipv6,
            )),
            Path => ambiguous.narrow(Host).map(|_| unreachable!()),
            // ^yield an error at the deciding character
        }
    }
}

impl<'src> HostSpan<'src> {
    pub(crate) fn new(src: &'src str) -> Result<Option<Self>, Error> {
        // handle bracketed ipv6 addresses
        if let Some(ambiguous) =
            HostOrPathSpan::new(src, HostKind::HostOrPath).map_err(disambiguate_err)?
        {
            Self::from_ambiguous(ambiguous).map(Some)
        } else {
            Ok(None)
        }
    }
    pub(crate) fn from_ambiguous(ambiguous: HostOrPathSpan<'src>) -> Result<Self, Error> {
        let kind = match ambiguous.kind() {
            HostKind::Host | HostKind::HostOrPath => Ok(Kind::Domain),
            HostKind::IpV6 => Ok(Kind::Ipv6),
            HostKind::Path => ambiguous.narrow(HostKind::Host).map(|_| unreachable!()),
            HostKind::Any => unreachable!("HostKind::Any should have been disambiguated"),
        }?;
        Ok(Self(Length::from_nonzero(ambiguous.short_len()), kind))
    }
}
impl<'src> From<Ipv6Span<'src>> for HostSpan<'src> {
    fn from(ipv6: Ipv6Span<'src>) -> Self {
        Self(Length::from_nonzero(ipv6.short_len()), Kind::Ipv6)
    }
}

pub struct HostStr<'src>(Kind, &'src str);
impl<'src> HostStr<'src> {
    pub fn src(&self) -> &'src str {
        self.1
    }
    pub fn kind(&self) -> Kind {
        self.0
    }
    #[inline]
    pub fn len(&self) -> usize {
        self.src().len()
    }
    #[inline]
    pub fn short_len(&self) -> NonZeroU8 {
        // unwrapping self.len() is safe since the length of a HostStr is always <= U::MAX
        let len: u8 = self.len().try_into().unwrap();
        // casting as nonzero is safe since the length of a HostStr is always > 0
        nonzero!(u8, len)
    }
    pub(super) fn from_span_of(src: &'src str, HostSpan(span, kind): HostSpan<'src>) -> Self {
        Self(kind, span.span_of(src))
    }
    pub fn from_prefix(src: &'src str) -> Result<Option<Self>, Error> {
        Ok(HostSpan::new(src)?.map(|span| Self::from_span_of(src, span)))
    }
    pub fn from_exact_match(src: &'src str) -> Result<Option<Self>, Error> {
        let result = HostSpan::new(src)?;
        let len = result.as_ref().map(|r| r.short_len().into()).unwrap_or(0);
        if (len as usize) != src.len() {
            return Err(Error::at(len, crate::err::Kind::HostInvalidChar));
        }
        Ok(result.map(|r| Self::from_span_of(src, r)))
    }
}
