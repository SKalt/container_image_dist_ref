// > name                            := [domain '/'] remote-name
// > domain                          := host [':' port-number]
// > port-number                     := /[0-9]+/
// > host                            := domain-name | IPv4address | \[ IPv6address \] ; rfc3986 appendix-A
// > domain-name                     := domain-component ['.' domain-component]*
// > domain-component                := alpha-numeric [ ( alpha-numeric | '-' )* alpha-numeric ]
// > path-component                  := alpha-numeric [separator alpha-numeric]*
// > path (or "remote-name")         := path-component ['/' path-component]*
// > alpha-numeric                   := /[a-z0-9]+/
// > separator                       := /[_.]|__|[-]*/
//
// Note that domain components conflict with path components:
// | class | domain-component | path-component |
// | ----- | ---------------- | -------------- |
// | upper | yes              | no             |
// | -     | inner            | inner          |
// | _     | no               | inner          |
// | .     | yes              | yes            |

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
use PortOrTagKind::{Port, Tag};

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
                left.short_len() as Long + right.short_len() as Long
            }
        }
    }
}

fn adapt_err(e: err::Error<Short>) -> err::Error<Long> {
    e.into()
}

impl<'src> DomainOrRefSpan<'src> {
    pub(crate) fn new(src: &'src str) -> Result<Self, Error> {
        let left = HostOrPathSpan::new(src, HostOrPathKind::Any)?
            .into_option()
            .ok_or(Error::at(0, err::Kind::HostOrPathNoMatch))?;
        let right_src = &src[left.len()..];
        let right_kind = if left.short_len() == Short::MAX {
            // no room left for a port in the name, since name is bounded at 255 chars
            // thus, the right side must be a Tag
            Tag
        } else {
            Port
        };
        let right = PortOrTagSpan::new(right_src, right_kind)
            .map_err(adapt_err)
            .map_err(|e| e + left.short_len())?;

        let len = left.len() + right.len();
        let rest = &src[len..];
        let next = rest.bytes().next();
        match next {
            Some(b'@') | None => {
                return PathSpan::from_ambiguous(left)
                    .map(|p| Self::TaggedRef((p, right.into())))
                    .map_err(adapt_err)
            } // needs to be a tagged ref no matter what
            Some(b'/') => match right.into_option().map(|r| r.kind()) {
                Some(_) => {
                    return DomainSpan::from_ambiguous(left, right, src)
                        .map(Self::Domain)
                        .map_err(adapt_err)
                }
                None => match left.kind() {
                    Path => {
                        // need to extend the path
                        let path = PathSpan::from_ambiguous(left)?
                            .extend(rest)
                            .map_err(adapt_err)
                            .map_err(|e| e + left.short_len())?;
                        return Ok(Self::TaggedRef((path, right.into())));
                    }
                    Host | IpV6 | HostOrPath => {
                        return DomainSpan::from_ambiguous(left, right, src)
                            .map(Self::Domain)
                            .map_err(adapt_err)
                    }
                    Any => unreachable!(
                        "HostOrPathSpan::new should always refine to a more specific kind"
                    ),
                },
            }, // needs to be a name
            _ => unreachable!(
                "PortOrTagSpan::new() only terminates successfully at '/', '@', or EOF"
            ),
        };
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
                    assert_eq!(domain.port().span_of(&src[left.len()..]), right);
                }
                DomainOrRefSpan::TaggedRef((path, tag)) => {
                    assert_eq!(path.span_of(src), left);
                    assert_eq!(tag.span_of(&src[left.len()..]), right);
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
        should_split("test.com:tag", "test.com", ":tag");
        should_split("test_com", "test_com", "");
    }
}
