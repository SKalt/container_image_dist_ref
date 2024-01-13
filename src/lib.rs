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
#![no_std]
pub(crate) mod ambiguous;
pub mod digest;
pub mod domain;
mod err;
pub mod path;
pub(crate) mod span;
mod tag;

/// the maximum total number of characters in a repository name, as defined by
/// https://github.com/distribution/reference/blob/main/reference.go#L39
pub const NAME_TOTAL_MAX_LENGTH: u8 = 255;

use core::ops::{Range, RangeFrom};

use self::{
    ambiguous::domain_or_tagged_ref::DomainOrRefSpan,
    digest::OptionalDigestSpan,
    domain::OptionalDomainSpan,
    path::OptionalPathSpan,
    span::{IntoOption, Lengthy, Long, Short},
    tag::TagSpan,
};
pub(crate) type Error = err::Error<Long>;
#[derive(PartialEq, Eq)]
struct RefSpan<'src> {
    domain: OptionalDomainSpan<'src>,
    path: OptionalPathSpan<'src>,
    tag: TagSpan<'src>,
    digest: OptionalDigestSpan<'src>,
}

impl<'src> RefSpan<'src> {
    pub fn new(src: &'src str) -> Result<Self, Error> {
        if src.is_empty() {
            return Error::at(0, err::Kind::RefNoMatch).into();
        };
        let prefix = DomainOrRefSpan::new(src)?;
        let domain = match prefix {
            DomainOrRefSpan::Domain(domain) => domain,
            DomainOrRefSpan::TaggedRef(_) => OptionalDomainSpan::none(),
        };
        let mut index: Long = domain.short_len().into();
        let rest = &src[index as usize..];
        let path = match prefix {
            DomainOrRefSpan::TaggedRef((left, _)) => Ok(left),
            DomainOrRefSpan::Domain(_) => match rest.bytes().next() {
                Some(b'/') => {
                    index += 1; // consume the leading '/'
                    OptionalPathSpan::new(&src[index as usize..]).map_err(|e| e.into())
                }
                Some(b'@') | None => Ok(OptionalPathSpan::none()),
                Some(_) => Error::at(0, err::Kind::PathInvalidChar).into(),
            },
        }
        .map_err(|e| e + index)?;
        index += path.short_len() as Long;
        if index > NAME_TOTAL_MAX_LENGTH.into() {
            return Error::at(index, err::Kind::NameTooLong).into();
        }
        let rest = &src[index as usize..];
        let tag = match prefix {
            DomainOrRefSpan::TaggedRef((_, right)) => match right.into_option() {
                Some(tag) => Ok(tag),
                None => match rest.bytes().next() {
                    Some(b':') => TagSpan::new(rest).map_err(|e| e.into()),
                    Some(b'@') | None => Ok(TagSpan::none()),
                    Some(_) => Error::at(0, err::Kind::PathInvalidChar).into(),
                },
            },
            DomainOrRefSpan::Domain(_) => match rest.bytes().next() {
                Some(b':') => TagSpan::new(rest).map_err(|e| e.into()),
                Some(_) | None => Ok(TagSpan::none()),
            },
        }
        .map_err(|e| e + index)?;
        index += tag.short_len() as Long;
        let rest = &src[index as usize..];
        let digest = match rest.bytes().next() {
            Some(b'@') => {
                index += 1;
                OptionalDigestSpan::new(&src[index as usize..])
            }
            Some(b) => unreachable!(
                "should have been caught by DomainOrRefSpan::new ; found {:?} @ {} in {:?}",
                b as char, index, src
            ),
            None => Ok(OptionalDigestSpan::none()),
        }
        .map_err(|e| e + index)?;
        index += digest.short_len();
        debug_assert!(
            index as usize == src.len(),
            "index {} != src.len() {}",
            index,
            src.len()
        );
        Ok(Self {
            domain,
            path,
            tag,
            digest,
        })
    }
    fn path_index(&self) -> usize {
        if let Some(d) = self.domain.into_option() {
            d.len() + 1 // add the trailing '/'
        } else {
            0
        }
    }
    fn tag_index(&self) -> usize {
        self.path_index() + self.path.len()
    }
    fn digest_index(&self) -> usize {
        self.tag_index() + self.tag.len() + self.digest.into_option().map(|_| 1).unwrap_or(0)
        // 1 == consume the leading '@' if a digest is present
    }

    fn domain_range(&self) -> Option<Range<usize>> {
        self.domain.into_option().map(|d| 0..d.len())
    }
    fn path_range(&self) -> Option<Range<usize>> {
        self.path
            .into_option()
            .map(|p| self.path_index()..self.path_index() + p.len())
    }
    fn name_range(&self) -> Option<Range<usize>> {
        let end = self.tag_index();
        if end == 0 {
            None
        } else {
            Some(0..end)
        }
    }
    fn tag_range(&self) -> Option<Range<usize>> {
        self.tag
            .into_option()
            .map(|t| self.tag_index() + 1..self.tag_index() + t.len())
        // 1 == consume the leading ':' if a tag is present
    }
    fn digest_range(&self) -> Option<RangeFrom<usize>> {
        self.digest.into_option().map(|_| self.digest_index()..)
    }
}

pub struct CanonicalSpan<'src>(RefSpan<'src>);
impl<'src> CanonicalSpan<'src> {
    pub fn new(src: &'src str) -> Result<Self, Error> {
        let domain = OptionalDomainSpan::new(src)?;
        let mut len = match src.as_bytes()[domain.len()..].iter().next() {
            Some(b'/') => Ok(domain.short_len().into()),
            Some(_) => Error::at(domain.short_len().into(), err::Kind::PathInvalidChar).into(),
            None => Error::at(domain.short_len().into(), err::Kind::RefNoMatch).into(),
        }?;
        let path = OptionalPathSpan::new(&src[len as usize..])
            .map_err(|e| e.into())
            .map_err(|e: err::Error<Long>| e + len)?;
        len += path.short_len() as Long;
        if len > NAME_TOTAL_MAX_LENGTH as u16 {
            return Error::at(len, err::Kind::NameTooLong).into();
        }
        let tag = TagSpan::new(&src[len as usize..]) // can be None
            .map_err(|e| e.into())
            .map_err(|e: err::Error<Long>| e + len)?;
        len += tag.short_len() as Long;
        len += match src.as_bytes()[len as usize..].iter().next() {
            Some(b'@') => Ok(1),
            Some(_) => Error::at(len, err::Kind::PathInvalidChar).into(),
            None => Error::at(len, err::Kind::RefNoMatch).into(),
        }?;
        let digest = OptionalDigestSpan::new(&src[len as usize..]).map_err(|e| e + len)?;
        Ok(Self(RefSpan {
            domain,
            path,
            tag,
            digest,
        }))
    }
}

pub struct RefStr<'src> {
    src: &'src str,
    // pub name: NameStr<'src>,
    span: RefSpan<'src>,
}
impl<'src> RefStr<'src> {
    pub fn new(src: &'src str) -> Result<Self, Error> {
        let span = RefSpan::new(src)?;
        Ok(Self { src, span })
    }

    pub fn domain(&self) -> Option<&str> {
        self.span.domain_range().map(|range| &self.src[range])
    }
    pub fn path(&self) -> Option<&str> {
        self.span.path_range().map(|range| &self.src[range])
    }
    pub fn name(&self) -> Option<&str> {
        self.span.name_range().map(|range| &self.src[range])
    }
    pub fn tag(&self) -> Option<&str> {
        self.span.tag_range().map(|range| &self.src[range])
    }
    pub fn digest(&self) -> Option<&str> {
        self.span.digest_range().map(|range| &self.src[range])
    }
}

/// produce an u8 representing the amount of information contained in the span
/// higher = more information, lower = less information
fn rank(span: &RefSpan) -> u8 {
    span.domain.into_option().map(|_| 1 << 3).unwrap_or(0)
        | span.path.into_option().map(|_| 1 << 2).unwrap_or(0)
        | span.tag.into_option().map(|_| 1 << 1).unwrap_or(0)
        | span.digest.into_option().map(|_| 1 << 0).unwrap_or(0)
}
// TODO: sort refs by information, most -> least
impl<'src> PartialOrd for RefSpan<'src> {
    fn partial_cmp(&self, other: &Self) -> Option<core::cmp::Ordering> {
        rank(other).partial_cmp(&rank(self))
        // note the order: if other has more information than self, then other
        // has to be ordered before than self
    }
}

pub struct CanonicalStr<'src> {
    src: &'src str,
    span: CanonicalSpan<'src>,
}
impl<'src> CanonicalStr<'src> {
    pub fn new(src: &'src str) -> Result<Self, Error> {
        let span = CanonicalSpan::new(src)?;
        Ok(Self { src, span })
    }
    pub fn domain(&self) -> &str {
        let domain = self.span.0.domain.span_of(self.src);
        debug_assert!(
            domain.len() > 0,
            "canonical refs should have non-empty domains by construction"
        );
        domain
    }
    pub fn path(&self) -> &str {
        let path = self
            .span
            .0
            .path
            .span_of(&self.src[self.span.0.path_index()..]);
        debug_assert!(
            path.len() > 0,
            "canonical refs should have non-empty paths by construction"
        );
        path
    }
    pub fn name(&self) -> &str {
        &self.src[self.span.0.name_range().unwrap()]
    }
    pub fn tag(&self) -> Option<&str> {
        self.span
            .0
            .tag
            .into_option()
            .map(|t| &t.span_of(&self.src[self.span.0.tag_index()..])[1..]) // trim the leading ':'
    }
    pub fn digest(&self) -> &str {
        let digest = &self.src[self.span.0.digest_range().unwrap()];
        debug_assert!(
            digest.len() > 0,
            "canonical refs should have non-empty digests by construction"
        );
        digest
    }
}

impl<'src> Into<RefStr<'src>> for CanonicalStr<'src> {
    fn into(self) -> RefStr<'src> {
        RefStr {
            src: self.src,
            span: self.span.0,
        }
    }
}
impl<'src> TryInto<CanonicalSpan<'src>> for RefSpan<'src> {
    type Error = Error;
    fn try_into(self) -> Result<CanonicalSpan<'src>, self::Error> {
        // a canonical reference needs a domain, path, and digest
        self.domain
            .into_option()
            .ok_or(Error::at(0, err::Kind::HostNoMatch))?;
        self.path.into_option().ok_or(Error::at(
            self.path_index().try_into().unwrap(),
            err::Kind::PathNoMatch,
        ))?;
        self.digest.into_option().ok_or(Error::at(
            self.digest_index().try_into().unwrap(),
            err::Kind::AlgorithmNoMatch, // TODO: more specific error?
        ))?;

        Ok(CanonicalSpan(self))
    }
}
impl<'src> TryInto<CanonicalStr<'src>> for RefStr<'src> {
    type Error = Error;
    fn try_into(self) -> Result<CanonicalStr<'src>, Self::Error> {
        CanonicalStr::new(self.src)
    }
}
#[cfg(test)]
mod tests {

    use super::*;
    fn should_parse(src: &str) -> RefStr {
        let result = RefStr::new(src);
        if let Err(e) = result {
            panic!(
                "failed to parse {:?}: {:?} @ {} ({:?})",
                src,
                e,
                e.index(),
                src.as_bytes()[e.1 as usize] as char
            );
        }
        let span = result.unwrap();
        span
    }
    fn should_parse_as(
        src: &str,
        domain: Option<&str>,
        path: Option<&str>,
        tag: Option<&str>,
        digest: Option<&str>,
    ) {
        let span = should_parse(src);
        let actual = (span.domain(), span.path(), span.tag(), span.digest());
        let expected = (domain, path, tag, digest);
        assert_eq!(actual, expected, "failed to parse {:?}", src);
    }
    #[test]
    fn test_name_only() {
        should_parse_as("test_com", None, Some("test_com"), None, None);
        should_parse_as("test.com", None, Some("test.com"), None, None)
    }
    #[test]
    fn test_tagged_ref() {
        should_parse_as("test.com:tag", None, Some("test.com"), Some("tag"), None);
        should_parse_as("test.com:5000", None, Some("test.com"), Some("5000"), None);
    }
    #[test]
    fn test_with_path() {
        should_parse_as(
            "test.com/repo:tag",
            Some("test.com"),
            Some("repo"),
            Some("tag"),
            None,
        );
        should_parse_as(
            "test:5000/repo",
            Some("test:5000"),
            Some("repo"),
            None,
            None,
        );
        should_parse_as(
            "test:5000/repo:tag",
            Some("test:5000"),
            Some("repo"),
            Some("tag"),
            None,
        )
    }
    #[test]
    fn test_with_digest() {
        // test:5000/repo@sha256:ffff
        should_parse_as(
            "host:5000/path:tag@algo:ffff",
            Some("host:5000"),
            Some("path"),
            Some("tag"),
            Some("algo:ffff"),
        );
        should_parse_as(
            "test@algo:bbbb",
            None,
            Some("test"),
            None,
            Some("algo:bbbb"),
        );
    }

    #[test]
    fn basic_corpus() {
        #[derive(Debug, PartialEq, Eq)]
        struct TestCase<'src> {
            input: &'src str,
            name: Option<&'src str>,
            domain: Option<&'src str>,
            path: Option<&'src str>,
            tag: Option<&'src str>,
            digest_algo: Option<&'src str>,
            digest_encoded: Option<&'src str>,
            err: Option<&'src str>,
        }
        impl<'src> From<&'src str> for TestCase<'src> {
            fn from(line: &'src str) -> Self {
                fn maybe(s: &str) -> Option<&str> {
                    if s.is_empty() {
                        None
                    } else {
                        Some(s)
                    }
                }
                let mut cols = line.split('\t');
                let input = cols.next().unwrap();
                let name = maybe(cols.next().unwrap());
                let domain = maybe(cols.next().unwrap());
                let path = maybe(cols.next().unwrap());
                let tag = maybe(cols.next().unwrap());
                let digest_algo = maybe(cols.next().unwrap());
                let digest_encoded = maybe(cols.next().unwrap());
                let err = maybe(cols.next().unwrap());
                Self {
                    input,
                    name,
                    domain,
                    path,
                    tag,
                    digest_algo,
                    digest_encoded,
                    err,
                }
            }
        }
        fn as_test_case<'s>(span: &'s RefStr<'s>) -> TestCase<'s> {
            let digest = span
                .digest()
                .map(|d| d.split(':'))
                .map(|mut iter| (iter.next().unwrap(), iter.next().unwrap()));
            TestCase {
                input: span.src,
                name: span.name(),
                domain: span.domain(),
                path: span.path(),
                tag: span.tag(),
                digest_algo: digest.map(|d| d.0),
                digest_encoded: digest.map(|d| d.1),
                err: None,
            }
        }

        fn expect(src: &str, expected: TestCase) {
            let parsed = RefStr::new(src);
            match (expected.err, parsed) {
                (Some(_err), Err(_e)) => {} // ok
                (None, Ok(actual)) => {
                    assert_eq!(as_test_case(&actual), expected)
                }
                (Some(err), Ok(_span)) => {
                    panic!("expected {src:?} to fail with {err:?}, but it succeeded")
                }
                (None, Err(e)) => panic!("expected {src:?} to succeed, but it failed with {e:?}"),
            }
        }
        let valid_inputs = include_str!("../tests/fixtures/references/valid/inputs.txt")
            .lines()
            .filter(|line| !line.is_empty());
        let invalid_inputs = include_str!("../tests/fixtures/references/invalid/inputs.txt")
            .lines()
            .filter(|line| !line.is_empty());
        let expected_outputs = include_str!("../tests/fixtures/references/outputs.tsv")
            .lines()
            .skip(1) // the header
            .filter(|line| !line.is_empty())
            .map(|line| TestCase::from(line));
        valid_inputs
            .chain(invalid_inputs)
            .zip(expected_outputs)
            .for_each(|(src, expected)| expect(src, expected))
    }
}
