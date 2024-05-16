//! # Host
//! Parses something like
//! ```txt
//! docker.io/library/alpine:3.14
//! ^^^^^^^^^
//! localhost:5000/registry/alpine:3.14
//! ^^^^^^^^^^^^^^
//! [2001:db8::1]:5000/registry/alpine:3.14
//! ^^^^^^^^^^^^^^^^^^
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
//! Note that host names CANNOT include underscores, which are reserved for
//! paths. This is a restriction that is not present in the URI spec (
//! [RFC 3986](https://www.rfc-editor.org/rfc/rfc3986#appendix-A)).

use core::num::NonZeroU8;

use crate::{
    ambiguous::host_or_path::{HostOrPathSpan, Kind as HostKind},
    err::{self},
    span::{impl_span_methods_on_tuple, nonzero, Length, Lengthy},
};

type Error = err::Error<u8>;

const fn disambiguate_err(e: Error) -> Error {
    let kind = match e.kind() {
        err::Kind::HostOrPathInvalidChar => err::Kind::HostInvalidChar,
        err::Kind::HostOrPathTooLong => err::Kind::HostTooLong,
        err::Kind::HostOrPathMissing => err::Kind::HostMissing,
        _ => e.kind(),
    };
    Error::at(e.index(), kind)
}

use super::ipv6::Ipv6Span;

#[allow(missing_docs)]
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Kind {
    /// a span of ascii characters that represents a restricted domain name, e.g. "Example.com".
    /// Must match the regex `^[a-zA-Z0-9][a-zA-Z0-9-]*[a-zA-Z0-9]$`
    Name,
    /// a restricted IPv6 address wrapped in square brackets, e.g. `[2001:db8::1]`
    /// Unlike the IPv6 described in [RFC 3986](https://www.rfc-editor.org/rfc/rfc3986#appendix-A),
    /// IPv4 mapping is forbidden: only hex digits and colons are allowed.
    Ipv6,
}

/// can be ipv6. Max length = ???
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) struct HostSpan<'src>(Length<'src, NonZeroU8>, Kind);
impl_span_methods_on_tuple!(HostSpan, u8, NonZeroU8);

impl<'src> HostSpan<'src> {
    /// Parses a host from the start of a string. Can be either a domain name or an IPv6 address.
    /// Can consume only part of the source string if it reaches a valid stopping point,
    /// i.e. `:`, `/`, or `@`.
    pub(crate) fn new(src: &'src str) -> Result<Self, Error> {
        let ambiguous = HostOrPathSpan::new(src, HostKind::Any).map_err(disambiguate_err)?;
        // handle bracketed ipv6 addresses
        Self::try_from(ambiguous)
    }
}

impl<'src> TryFrom<HostOrPathSpan<'src>> for HostSpan<'src> {
    type Error = Error;
    fn try_from(ambiguous: HostOrPathSpan) -> Result<Self, Error> {
        let kind = match ambiguous.kind() {
            HostKind::Host | HostKind::HostOrPath => Ok(Kind::Name),
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
/// An underscore-free host name or a bracketed IPv6 address.
///
/// # Examples
///
/// ```rust
/// use container_image_dist_ref::name::domain::{Host, Kind::*};
/// let host = Host::new("docker.io").unwrap();
/// assert_eq!(host.kind(), Name);
/// assert_eq!(host.to_str(), "docker.io");
///
/// let host = Host::new("[2001:db8::1]").unwrap();
/// assert_eq!(host.kind(), Ipv6);
/// assert_eq!(host.to_str(), "[2001:db8::1]");
/// ```
pub struct Host<'src>(Kind, &'src str);
#[allow(clippy::len_without_is_empty)]
impl<'src> Host<'src> {
    #[allow(missing_docs)]
    pub const fn to_str(&self) -> &'src str {
        self.1
    }
    /// ipb6 or domain
    pub const fn kind(&self) -> Kind {
        self.0
    }
    #[allow(missing_docs)]
    #[inline]
    pub const fn len(&self) -> usize {
        self.to_str().len()
    }
    #[inline]
    #[allow(clippy::unwrap_used)]
    fn short_len(&self) -> NonZeroU8 {
        // unwrapping self.len() is safe since the length of a Host is always <= U::MAX
        let len: u8 = self.len().try_into().unwrap();
        // casting as nonzero is safe since the length of a Host is always > 0
        nonzero!(u8, len)
    }
    pub(super) fn from_span(src: &'src str, HostSpan(span, kind): HostSpan<'src>) -> Self {
        debug_assert_eq!(span.len(), src.len(), "{src:?}");
        Self(kind, span.span_of(src))
    }
    /// Parse a valid host from the start of the string. Parsing may not consume the entire string
    /// if it reaches a valid stopping point, i.e. `:`, `/`, or `@`.
    pub fn new(src: &'src str) -> Result<Self, Error> {
        let span = HostSpan::new(src)?;
        Ok(Self::from_span(src, span))
    }
    /// checks that the entire source string is consumed
    pub fn from_exact_match(src: &'src str) -> Result<Self, Error> {
        let result = Self::new(src)?;
        if result.len() != src.len() {
            return Err(Error::at(
                result.short_len().into(),
                crate::err::Kind::HostInvalidChar,
            ));
        }
        Ok(result)
    }
}
