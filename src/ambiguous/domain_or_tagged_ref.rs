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
    domain::DomainSpan,
    err,
    path::PathSpan,
    span::{Lengthy, OptionallyZero},
    tag::TagSpan,
};
use HostOrPathKind::{Any, Host, HostOrPath, IpV6, Path};
use PortOrTagKind::Port;

pub(crate) type Error = err::Error<u16>;
/// represents a colon-delimited string of the form "left:right"
pub(crate) enum DomainOrRefSpan<'src> {
    Domain(DomainSpan<'src>),
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
                    len = len.checked_add(1).unwrap(); // add 1 for the leading ':'
                    len = len.checked_add(tag.short_len().upcast().into()).unwrap();
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
fn map_err(e: err::Error<u8>) -> err::Error<u16> {
    e.into()
}

impl<'src> DomainOrRefSpan<'src> {
    pub(crate) fn new(src: &'src str) -> Result<Self, Error> {
        let left = HostOrPathSpan::new(src, HostOrPathKind::Any)?
            .ok_or(Error::at(0, err::Kind::HostOrPathNoMatch))?;
        let right_src = &src[left.len()..];
        let right = match right_src.bytes().next() {
            Some(b':') => {
                let len = left.short_len().widen().upcast() + 1;
                // +1 for the leading ':'
                let right = PortOrTagSpan::new(&right_src[1..], Port)
                    .map_err(map_err)
                    .map_err(|e| e + len)?;
                Ok(Some(
                    right.ok_or(Error::at(len, err::Kind::PortOrTagMissing))?,
                ))
            }
            Some(b'/') | Some(b'@') | None => Ok(None),
            Some(_) => Error::at(
                left.short_len().upcast().into(),
                err::Kind::PortOrTagInvalidChar,
            )
            .into(),
        }?;

        let len = left.len() + right.map(|r| r.len() + 1).unwrap_or(0); // +1 for the leading ':'
        let rest = &src[len..];
        match rest.bytes().next() {
            Some(b'@') | None => {
                // since the next section must be a digest, the right side must be a tag
                let path = PathSpan::from_ambiguous(left).map_err(map_err)?;
                let tag = if let Some(tag) = right {
                    Some(
                        tag.try_into()
                            .map_err(map_err)
                            .map_err(|e| e + path.short_len().upcast())
                            .map_err(|e| e + 1u16)?, // the only error is TagTooLong, which gets thrown if there's a tag. Thus, add +1 for the leading ':'
                    )
                    // addition is safe since path can be at most 255ch and tag can be at most 128ch
                } else {
                    None
                };
                Ok(Self::TaggedRef((path, tag)))
            }
            Some(b'/') => {
                // needs to be a name
                return if right.is_some() {
                    // right must be a port, so left must be a domain
                    DomainSpan::from_ambiguous(left, right).map(Self::Domain)
                } else {
                    match left.kind() {
                        Path => {
                            // need to extend the path
                            let path = PathSpan::from_ambiguous(left)?
                                .extend(rest)
                                .map_err(map_err)?;

                            let tag = if let Some(t) = right {
                                Some(
                                    t.try_into()
                                    .map_err(map_err)
                                    .map_err(|e| e + 1u8) // +1 for the leading ':'
                                    .map_err(|e| e + path.short_len().upcast())?,
                                )
                            } else {
                                None
                            };
                            Ok(Self::TaggedRef((path, tag)))
                        }
                        Host | IpV6 | HostOrPath => {
                            DomainSpan::from_ambiguous(left, right).map(Self::Domain)
                        }
                        Any => {
                            return Err(Error::at(
                                len.try_into().unwrap(),
                                err::Kind::HostOrPathNoMatch,
                            ))
                        }
                    }
                };
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
