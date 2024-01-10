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

// FIXME: distinguish between offsets and lengths

use self::{
    ambiguous::{domain_or_tagged_ref::DomainOrRefSpan, port_or_tag::OptionalPortOrTag},
    digest::{Compliance, OptionalDigestSpan},
    domain::OptionalDomainSpan,
    err::Error,
    path::OptionalPathSpan,
    span::{IntoOption, Span, SpanMethods, MAX_USIZE, U},
    tag::OptionalTagSpan,
};

pub struct Reference<'src> {
    src: &'src str,
    // pub name: NameStr<'src>,
    optional_domain: OptionalDomainSpan<'src>,
    path: OptionalPathSpan<'src>,
    pub tag: OptionalPortOrTag<'src>,
    pub digest: OptionalDigestSpan<'src>,
}

struct RefSpan<'src> {
    domain: OptionalDomainSpan<'src>,
    path: OptionalPathSpan<'src>,
    tag: OptionalTagSpan<'src>,
    digest: OptionalDigestSpan<'src>,
    digest_compliance: Compliance,
}

impl<'src> RefSpan<'src> {
    pub fn new(src: &'src str) -> Result<Self, Error> {
        match src.len() {
            1..=MAX_USIZE => Ok(()), // check length addressable by integer size
            0 => Err(Error(err::Kind::RefNoMatch, 0)),
            _ => Err(Error(err::Kind::RefTooLong, U::MAX)),
        }?;
        let prefix = DomainOrRefSpan::new(src)?;
        let domain = match prefix {
            DomainOrRefSpan::Domain(domain) => domain,
            DomainOrRefSpan::TaggedRef(_) => OptionalDomainSpan::none(),
        };
        let mut index = domain.len();
        let rest = &src[index..];
        let path = match prefix {
            DomainOrRefSpan::TaggedRef((left, _)) => Ok(left),
            DomainOrRefSpan::Domain(_) => match rest.bytes().next() {
                Some(b'/') => OptionalPathSpan::parse_from_slash(&src[index..]),
                Some(b'@') | None => Ok(OptionalPathSpan::none()),
                Some(_) => Err(Error(err::Kind::PathInvalidChar, 0)),
            },
        }
        .map_err(|e| e + domain.short_len())?;
        index += path.len();
        let rest = &src[index..];
        let tag = match prefix {
            DomainOrRefSpan::TaggedRef((_, right)) => match right.into_option() {
                Some(tag) => Ok(tag),
                None => match rest.bytes().next() {
                    Some(b':') => OptionalTagSpan::new(rest),
                    Some(b'@') | None => Ok(OptionalTagSpan::none()),
                    Some(_) => Err(Error(err::Kind::PathInvalidChar, index.try_into().unwrap())),
                },
            },
            DomainOrRefSpan::Domain(_) => match src[prefix.len() + path.len()..].bytes().next() {
                Some(b':') => OptionalTagSpan::new(rest),
                Some(_) | None => Ok(OptionalTagSpan::none()),
            },
        }
        .map_err(|e| e + prefix.short_len() + path.short_len())?;
        index += tag.len();
        let rest = &src[index..];
        // FIXME: have DigestSpan own the leading '@'
        let (digest, compliance) = match rest.bytes().next() {
            Some(b'@') => OptionalDigestSpan::new(&rest[1..]),
            Some(b) => unreachable!(
                "should have been caught by DomainOrRefSpan::new ; found {:?} @ {} in {:?}",
                b as char, index, src
            ),
            None => Ok((OptionalDigestSpan::none(), Compliance::Universal)),
        }
        .map_err(|e| e + prefix.short_len() + path.short_len() + tag.short_len())?;
        Ok(Self {
            domain,
            path,
            tag,
            digest,
            digest_compliance: compliance,
        })
    }
}

#[cfg(test)]
mod tests {
    fn should_parse(src: &str) {
        let result = super::RefSpan::new(src);
        if let Err(e) = result {
            panic!(
                "failed to parse {:?}: {:?} @ {} ({:?})",
                src,
                e,
                e.1,
                src.as_bytes()[e.1 as usize] as char
            );
        }
    }

    #[test]
    fn basic_corpus() {
        include_str!("../tests/fixtures/references/valid/inputs.txt")
            .lines()
            .filter(|line| !line.is_empty())
            .for_each(|line| should_parse(line));
    }
}
