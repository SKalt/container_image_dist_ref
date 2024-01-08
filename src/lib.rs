// > Grammar
// >
// > ```ebnf
// > reference                       := name [ ":" tag ] [ "@" digest ]
// > name                            := [domain '/'] remote-name
// > domain                          := host [':' port-number]
// > host                            := domain-name | IPv4address | \[ IPv6address \] ; rfc3986 appendix-A
// > domain-name                     := domain-component ['.' domain-component]*
// > domain-component                := /([a-zA-Z0-9]|[a-zA-Z0-9][a-zA-Z0-9-]*[a-zA-Z0-9])/
// > port-number                     := /[0-9]+/
// > path-component                  := alpha-numeric [separator alpha-numeric]*
// > path (or "remote-name")         := path-component ['/' path-component]*
// > alpha-numeric                   := /[a-z0-9]+/
// > separator                       := /[_.]|__|[-]*/
// >
// > tag                             := /[\w][\w.-]{0,127}/
// >
// > digest                          := digest-algorithm ":" digest-hex
// > digest-algorithm                := digest-algorithm-component [ digest-algorithm-separator digest-algorithm-component ]*
// > digest-algorithm-separator      := /[+.-_]/
// > digest-algorithm-component      := /[A-Za-z][A-Za-z0-9]*/
// > digest-hex                      := /[0-9a-fA-F]{32,}/ ; At least 128 bit digest value
// >
// > identifier                      := /[a-f0-9]{64}/
// > ```
// >
// > -- https://github.com/distribution/reference/blob/v0.5.0/reference.go#L4-L26
// > -- https://github.com/distribution/reference/blob/4894124079e525c3c3c5c8aacaa653b5499004e9/reference.go#L4-L26
// https://www.rfc-editor.org/rfc/rfc3986#appendix-A
// https://docs.rs/regex/latest/regex/#sharing-a-regex-across-threads-can-result-in-contention
// #![no_std]
pub(crate) mod ambiguous;
pub mod digest;
pub mod domain;
mod err;
pub mod path;
pub(crate) mod span;
mod tag;

use ambiguous::domain_or_tagged_ref::DomainOrRef;
use domain::OptionalDomainSpan;
use span::{SpanMethods, MAX_USIZE};
use tag::OptionalTagSpan;

use self::{
    ambiguous::port_or_tag::OptionalPortOrTag,
    digest::{Compliance, OptionalDigestSpan},
    domain::DomainSpan,
    err::Error,
    path::PathSpan,
    span::{IntoOption, Span, U},
};

pub struct Reference<'src> {
    src: &'src str,
    // pub name: NameStr<'src>,
    optional_domain: DomainSpan<'src>,
    path: PathSpan<'src>,
    pub tag: OptionalPortOrTag<'src>,
    pub digest: OptionalDigestSpan<'src>,
}

struct RefSpan<'src> {
    optional_domain: OptionalDomainSpan<'src>,
    path: PathSpan<'src>,
    optional_tag: OptionalTagSpan<'src>,
    optional_digest: OptionalDigestSpan<'src>,
}

impl<'src> RefSpan<'src> {
    pub fn new(src: &'src str) -> Result<(Self, Compliance), Error> {
        match src.len() {
            1..=MAX_USIZE => Ok(()), // check length addressable by integer size
            0 => Err(Error(err::Kind::RefNoMatch, 0)),
            _ => Err(Error(err::Kind::RefTooLong, U::MAX)),
        }?;
        // !!!!!
        let first_bit = DomainOrRef::new(src);
        if let Ok(first_bit) = first_bit {
            match src[first_bit.len()..].bytes().next() {
                Some(b'/') => {
                    let domain: OptionalDomainSpan<'src> = first_bit.try_into()?;
                    // consume the separator slash
                    let mut len = domain.len() + 1;
                    let path = PathSpan::new(&src[len..])?;
                    len += path.len();
                    let optional_tag = OptionalTagSpan::new(&src[len..])?;
                    len += optional_tag.len();
                    let (optional_digest, compliance) = OptionalDigestSpan::new(&src[len..])?;
                    Ok((
                        Self {
                            optional_domain: domain,
                            path,
                            optional_tag,
                            optional_digest,
                        },
                        compliance,
                    ))
                }
                None => {
                    let path = first_bit.host_or_path.try_into()?;
                    let tag = first_bit.optional_port_or_tag.try_into()?;
                    Ok((
                        Self {
                            optional_domain: OptionalDomainSpan::none(),
                            path,
                            optional_tag: tag,
                            optional_digest: OptionalDigestSpan::none(),
                        },
                        Compliance::Universal,
                    ))
                }
                _ => Err(Error(err::Kind::RefNoMatch, first_bit.short_len())),
            }
        } else {
            // something's unambiguously wrong, so treat it like a domain
            // and surface the error
            let _domain: OptionalDomainSpan<'src> = first_bit.try_into()?;
            unreachable!()
        }
    }
}
