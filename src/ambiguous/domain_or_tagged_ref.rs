//! # ambiguous domain or tagged ref
//! structs in this parse either a domain or a tagged ref into an enum, then
//! let the caller decide what to do with it.
//!
//! Note that domain components conflict with path components:

// {{{sh
//    cd ../../ && ./scripts/lines.sh 1 12 ./grammars/reference.ebnf |
//    sed 's#^#//! #g';
//    printf '//! ```\n\n// ';
// }}}{{{out skip=2

//! ```ebnf
//! reference            ::= name (":" tag )? ("@" digest )?
//! name                 ::= (domain "/")? path
//! domain               ::= host (":" port-number)?
//! host                 ::= domain-name | IPv4address | "[" IPv6address "]" /* see https://www.rfc-editor.org/rfc/rfc3986#appendix-A */
//! domain-name          ::= domain-component ("." domain-component)*
//! domain-component     ::= ([a-zA-Z0-9]|[a-zA-Z0-9][a-zA-Z0-9-]*[a-zA-Z0-9])
//! port-number          ::= [0-9]+
//! path-component       ::= [a-z0-9]+ (separator [a-z0-9]+)*
//! path                 ::= path-component ("/" path-component)*
//! separator            ::= [_.] | "__" | "-"+
//!
//! tag                  ::= [\w][\w.-]{0,127}
//! ```

// }}}

use core::num::NonZeroU16;

use crate::{
    ambiguous::{
        host_or_path::{HostOrPathSpan, Kind as HostOrPathKind},
        port_or_tag::{Kind as PortOrTagKind, PortOrTagSpan},
    },
    err,
    name::domain::DomainSpan,
    path::PathSpan,
    span::{Lengthy, OptionallyZero},
    tag::TagSpan,
};
use HostOrPathKind::{Any, Host, HostOrPath, IpV6, Path};
use PortOrTagKind::Port;

pub(crate) type Error = err::Error<u16>;
/// represents a colon-delimited string of the form `left:right`, with a max possible length
/// of 255+1+128 = 384
pub(crate) enum DomainOrRefSpan<'src> {
    /// A span that must be a domain since either:
    /// - it's started by an IPv6 address
    /// - it's followed by a `/`
    Domain(DomainSpan<'src>),
    /// A span that must be a path since either:
    /// - its left side contains underscores
    /// - it contains a tag with non-digit characters
    /// - it's followed by a `@`
    TaggedRef((PathSpan<'src>, Option<TagSpan<'src>>)),
}

impl Lengthy<'_, u16, NonZeroU16> for DomainOrRefSpan<'_> {
    fn short_len(&self) -> NonZeroU16 {
        match self {
            DomainOrRefSpan::Domain(d) => d.short_len(),
            DomainOrRefSpan::TaggedRef((left, right)) => {
                let mut len =
                    unsafe { NonZeroU16::new_unchecked(u8::from(left.short_len()) as u16) };
                if let Some(tag) = right {
                    // safe to unwrap since left can be at most 255 and right can be at most 128
                    len = len.saturating_add(1); // add 1 for the leading ':'
                    len = len.saturating_add(tag.short_len().upcast().into());
                }
                len
            }
        }
    }
    #[inline]
    fn len(&self) -> usize {
        self.short_len().as_usize()
    }
}

#[inline]
fn to_large_err(e: err::Error<u8>) -> err::Error<u16> {
    e.into()
}

impl<'src> DomainOrRefSpan<'src> {
    pub(crate) fn new(src: &'src str) -> Result<Self, Error> {
        let left = HostOrPathSpan::new(src, HostOrPathKind::Any)?;
        let right_src = &src[left.len()..]; // TODO: consolidate with rest
        let mut len = left.short_len().widen().upcast(); // current possible max: 255
        let right = match right_src.bytes().next() {
            Some(b'/') | Some(b'@') | None => None,
            Some(b':') => {
                len = len.saturating_add(1); // +1 for the ':'
                let right = PortOrTagSpan::new(&right_src[1..], Port).map_err(|e| {
                    Error::at(
                        len.saturating_add(e.index() as u16), // ok since len <= 256, so len + u8::MAX < u16::MAX
                        e.kind(),
                    )
                })?;
                Some(right)
            }
            Some(_) => {
                return Err(Error::at(
                    left.short_len().widen().into(),
                    err::Kind::PortOrTagInvalidChar,
                ))
            }
        };

        len = len.saturating_add(right.map(|r| r.short_len().widen().upcast()).unwrap_or(0));
        let rest = &src[len as usize..];
        match rest.bytes().next() {
            Some(b'@') | None => {
                // since the next section must be a digest, the right side must be a tag
                let path = PathSpan::try_from(left).map_err(to_large_err)?;
                let tag = if let Some(tag) = right {
                    Some(tag.try_into().map_err(|e: err::Error<u8>| {
                        Error::at(
                            path.short_len()
                                        .widen()
                                        .upcast()
                                        .saturating_add(1u16) // for the leading ':'
                                        .saturating_add(e.index() as u16),
                            e.kind(),
                        )
                    })?)
                    // addition is safe since path can be at most 255ch and tag can be at most 128ch
                } else {
                    None
                };
                Ok(Self::TaggedRef((path, tag)))
            }
            Some(b'/') => {
                // needs to be a name
                if right.is_some() {
                    // right must be a port, so left must be a domain
                    DomainSpan::from_ambiguous(left, right).map(Self::Domain)
                } else {
                    match left.kind() {
                        Path => {
                            // need to extend the path
                            let path = PathSpan::try_from(left)?
                                .extend(rest)
                                .map_err(to_large_err)?;

                            let tag = if let Some(t) = right {
                                Some(t.try_into().map_err(|e: err::Error<u8>| {
                                    Error::at(
                                        path.short_len()
                                        .widen()
                                        .upcast()
                                        .saturating_add(1u16) // for the leading ':'
                                        .saturating_add(e.index() as u16),
                                        e.kind(),
                                    )
                                })?)
                            } else {
                                None
                            };
                            Ok(Self::TaggedRef((path, tag)))
                        }
                        Host | IpV6 | HostOrPath => {
                            DomainSpan::from_ambiguous(left, right).map(Self::Domain)
                        }
                        Any => Error::at(len, err::Kind::HostOrPathMissing).into(),
                    }
                }
            }
            _ => unreachable!(
                "PortOrTagSpan::new() only terminates successfully at '/', '@', or EOF"
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::span::Lengthy;
    fn should_split(src: &str, left: &str, right: &str) {
        let tag = DomainOrRefSpan::new(src);
        match tag {
            Ok(span) => match span {
                DomainOrRefSpan::Domain(domain) => {
                    assert_eq!(domain.host.span_of(src), left);
                    if let Some(p) = domain.port {
                        assert_eq!(p.span_of(&src[left.len() + 1..]), right);
                    } else {
                        assert_eq!(right, "");
                    };
                    assert_eq!(
                        domain
                            .port
                            .map(|p| p.span_of(&src[left.len() + 1..]))
                            .unwrap_or(src),
                        right
                    );
                }
                DomainOrRefSpan::TaggedRef((path, tag)) => {
                    assert_eq!(path.span_of(src), left);
                    if let Some(t) = tag {
                        assert_eq!(t.span_of(&src[left.len() + 1..]), right);
                    } else {
                        assert_eq!(right, "");
                    };
                }
            },
            Err(e) => {
                let index = e.index() as usize;
                assert!(
                    index < src.len(),
                    "error {:?} @ index {index} is greater than src.len() {}",
                    e.kind(),
                    src.len()
                );
                panic!("{src:?} -> {e:?} :: {:?}", src.as_bytes()[index] as char);
            }
        }
    }

    #[test]
    fn test_ambiguous() {
        should_split("test.com:tag", "test.com", "tag");
        should_split("test_com", "test_com", "");
    }
}
