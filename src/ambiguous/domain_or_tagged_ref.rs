//! # ambiguous domain or tagged ref
//! structs in this parse either a domain or a tagged ref into an enum, then
//! let the caller decide what to do with it.
//!
//! Note that domain components conflict with path components:

// {{{sh
//    cd ../../../ && ./scripts/lines.sh 1 12 ./grammars/reference.ebnf |
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

use crate::{
    ambiguous::{
        host_or_path::{HostOrPathSpan, Kind as HostOrPathKind},
        port_or_tag::{Kind as PortOrTagKind, PortOrTagSpan},
    },
    domain::DomainSpan,
    err,
    path::PathSpan,
    span::{IntoOption, Lengthy, Long, Short},
    tag::TagSpan,
};
use HostOrPathKind::{Any, Host, HostOrPath, IpV6, Path};
use PortOrTagKind::Port;

pub(crate) type Error = err::Error<Long>;
/// represents a colon-delimited string of the form "left:right"
pub(crate) enum DomainOrRefSpan<'src> {
    Domain(DomainSpan<'src>),
    TaggedRef((PathSpan<'src>, TagSpan<'src>)),
}

impl Lengthy<'_, Long> for DomainOrRefSpan<'_> {
    fn short_len(&self) -> Long {
        match self {
            DomainOrRefSpan::Domain(d) => d.short_len().into(),
            DomainOrRefSpan::TaggedRef((left, right)) => {
                left.short_len() as Long
                    + right
                        .into_option()
                        .map(|t| t.short_len() as Long)
                        .map(|l| l + 1) // add after padding to prevent overflow of Short
                        .unwrap_or(0)
            }
        }
    }
}

// TODO: check whether this has any effect on performance
fn map_err(e: err::Error<Short>) -> err::Error<Long> {
    e.into()
}

impl<'src> DomainOrRefSpan<'src> {
    pub(crate) fn new(src: &'src str) -> Result<Self, Error> {
        let left = HostOrPathSpan::new(src, HostOrPathKind::Any)?
            .into_option()
            .ok_or(Error::at(0, err::Kind::HostOrPathNoMatch))?;
        let right_src = &src[left.len()..];
        let right = match right_src.bytes().next() {
            Some(b':') => {
                PortOrTagSpan::new(&right_src[1..], Port)
                    .map_err(map_err)
                    .map_err(|e| e + 1u8) // +1 for the leading ':'
                    .map_err(|e| e + left.short_len())
            }
            Some(b'/') | Some(b'@') | None => Ok(PortOrTagSpan::none()),
            Some(_) => Error::at(left.short_len().into(), err::Kind::PortOrTagInvalidChar).into(),
        }?;

        let len = left.len() + right.into_option().map(|r| r.len() + 1).unwrap_or(0); // +1 for the leading ':'
        let rest = &src[len..];
        match rest.bytes().next() {
            Some(b'@') | None => {
                // since the next section must be a digest, the right side must be a tag
                let path = PathSpan::from_ambiguous(left).map_err(map_err)?;
                Ok(Self::TaggedRef((
                    path,
                    right
                        .try_into()
                        .map_err(map_err)
                        .map_err(|e| e + 1u16) // the only error is TagTooLong, which gets thrown if there's a tag. Thus, add +1 for the leading ':'
                        .map_err(|e| e + path.short_len())?, // addition is safe since path can be at most 255ch and tag can be at most 128ch
                )))
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
                                .map_err(map_err)
                                .map_err(|e| e + left.short_len())?;
                            let tag = right
                                .try_into()
                                .map_err(map_err)
                                .map_err(|e| e + 1u8) // +1 for the leading ':'
                                .map_err(|e| e + path.short_len())?;
                            Ok(Self::TaggedRef((path, tag)))
                        }
                        Host | IpV6 | HostOrPath => {
                            DomainSpan::from_ambiguous(left, right).map(Self::Domain)
                        }
                        Any => unreachable!(
                            "HostOrPathSpan::new should always refine to a more specific kind"
                        ),
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
                    assert_eq!(domain.host().span_of(src), left);
                    if let Some(p) = domain.port().into_option() {
                        assert_eq!(p.span_of(&src[left.len() + 1..]), right);
                    } else {
                        assert_eq!(right, "");
                    };
                    assert_eq!(domain.port().span_of(&src[left.len() + 1..]), right);
                }
                DomainOrRefSpan::TaggedRef((path, tag)) => {
                    assert_eq!(path.span_of(src), left);
                    if let Some(t) = tag.into_option() {
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
