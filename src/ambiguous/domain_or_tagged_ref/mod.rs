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
use HostOrPathKind::{Either as EitherHostPathOrIpv6, Host, IpV6, Path};
use PortOrTagKind::{Either as EitherPortOrTag, Port, Tag};

pub(crate) type Error = err::Error<Long>;
/// represents a colon-delimited string of the form "left:right"
pub(crate) enum DomainOrRefSpan<'src> {
    Domain(DomainSpan<'src>),
    TaggedRef((PathSpan<'src>, TagSpan<'src>)),
}

pub(crate) enum Kind {
    Domain,
    TaggedRef,
}
impl Lengthy<'_, Short> for DomainOrRefSpan<'_> {
    fn short_len(&self) -> Short {
        match self {
            DomainOrRefSpan::Domain(d) => d.short_len(),
            DomainOrRefSpan::TaggedRef((left, right)) => left.short_len() + right.short_len(),
        }
    }
}

impl<'src> DomainOrRefSpan<'src> {
    pub(crate) fn new(src: &'src str) -> Result<Self, Error> {
        let left = HostOrPathSpan::new(src, EitherHostPathOrIpv6)?;
        let right_src = &src[left.len()..];
        let right = match right_src.bytes().next() {
            Some(b':') => PortOrTagSpan::new(
                right_src,
                if left.short_len() == Short::MAX {
                    // no room left for a port in the name, since name is bounded at 255 chars
                    // thus, the right side must be a Tag
                    Tag
                } else {
                    EitherPortOrTag
                },
            ),
            Some(b'/') | Some(b'@') | None => Ok(PortOrTagSpan::none()),
            Some(_) => err::Error::<Short>::at(0, err::Kind::HostOrPathInvalidChar).into(),
        }
        .map_err(|e: err::Error<Short>| -> Error { e.into() })
        .map_err(|e| e + left.short_len())?;

        let kind = Self::infer_kind_from_suffix(src[left.len() + right.len()..].bytes().next())?;
        Self::from_parts(left, right, kind, src)
    }
    fn infer_kind_from_suffix(next_ascii_char: Option<u8>) -> Result<Kind, Error> {
        match next_ascii_char {
            Some(b'/') => Ok(Kind::Domain),
            None | Some(b'@') => Ok(Kind::TaggedRef),
            Some(_) => Error::at(0, err::Kind::HostOrPathInvalidChar).into(),
        }
    }
    pub(crate) fn from_parts(
        left: HostOrPathSpan<'src>,
        right: PortOrTagSpan<'src>,
        target: Kind,
        context: &'src str,
    ) -> Result<Self, Error> {
        let left_kind = match target {
            Kind::Domain => match left.kind() {
                IpV6 => IpV6,
                _ => Host,
            },
            Kind::TaggedRef => Path,
        };
        let left = left.narrow(left_kind, context)?;
        let right_kind = match target {
            Kind::Domain => Port,
            Kind::TaggedRef => Tag,
        };
        let right = right
            .narrow(right_kind, &context[left.len()..])
            .map_err(|e| e + left.short_len())?;
        match target {
            Kind::Domain => Ok(Self::Domain(DomainSpan::from_ambiguous_parts(
                left, right, context,
            )?)),
            Kind::TaggedRef => Ok(Self::TaggedRef((
                PathSpan::from_ambiguous(left, context)?,
                right.into(),
            ))),
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
