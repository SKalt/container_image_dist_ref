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

use self::{
    ambiguous::domain_or_tagged_ref::DomainOrRefSpan,
    digest::OptionalDigestSpan,
    domain::OptionalDomainSpan,
    path::OptionalPathSpan,
    span::{IntoOption, Lengthy, Long, Short},
    tag::TagSpan,
};
pub(crate) type Error = err::Error<Long>;
struct RefSpan<'src> {
    domain: OptionalDomainSpan<'src>,
    path: OptionalPathSpan<'src>,
    tag: TagSpan<'src>,
    digest: OptionalDigestSpan<'src>,
}

impl<'src> RefSpan<'src> {
    pub fn new(src: &'src str) -> Result<Self, Error> {
        if src.is_empty() {
            return Error::at(0, err::Kind::RefNoMatch);
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
                Some(_) => Error::at(0, err::Kind::PathInvalidChar),
            },
        }
        .map_err(|e| e + index)?;
        index += path.short_len() as Long;
        if index > NAME_TOTAL_MAX_LENGTH.into() {
            return Error::at(index, err::Kind::NameTooLong);
        }
        let rest = &src[index as usize..];
        let tag = match prefix {
            DomainOrRefSpan::TaggedRef((_, right)) => match right.into_option() {
                Some(tag) => Ok(tag),
                None => match rest.bytes().next() {
                    Some(b':') => TagSpan::new(rest).map_err(|e| e.into()),
                    Some(b'@') | None => Ok(TagSpan::none()),
                    Some(_) => Error::at(0, err::Kind::PathInvalidChar),
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
}

pub struct Reference<'src> {
    src: &'src str,
    // pub name: NameStr<'src>,
    span: RefSpan<'src>,
}
impl<'src> Reference<'src> {
    pub fn new(src: &'src str) -> Result<Self, Error> {
        let span = RefSpan::new(src)?;
        Ok(Self { src, span })
    }

    fn path_index(&self) -> usize {
        if let Some(d) = self.span.domain.into_option() {
            d.len() + 1 // add the trailing '/'
        } else {
            0
        }
    }

    fn tag_index(&self) -> usize {
        self.path_index() + self.span.path.len()
    }
    fn digest_index(&self) -> usize {
        self.tag_index()
            + self.span.tag.len()
            + self.span.digest.into_option().map(|_| 1).unwrap_or(0)
        // consume the leading '@' if a digest is present
    }

    pub fn domain(&self) -> Option<&str> {
        self.span.domain.into_option().map(|d| d.span_of(self.src))
    }
    pub fn path(&self) -> Option<&str> {
        self.span
            .path
            .into_option()
            .map(|p| p.span_of(&self.src[self.path_index()..]))
    }
    pub fn name(&self) -> &str {
        &self.src[..self.tag_index()]
    }
    pub fn tag(&self) -> Option<&str> {
        self.span
            .tag
            .into_option()
            .map(|t| &t.span_of(&self.src[self.tag_index()..])[1..]) // trim the leading ':'
    }
    pub fn digest(&self) -> Option<&str> {
        self.span
            .digest
            .into_option()
            .map(|d| d.span_of(&self.src[self.digest_index()..]))
    }
}

#[cfg(test)]
mod tests {

    use super::*;
    fn should_parse(src: &str) -> Reference {
        let result = Reference::new(src);
        if let Err(e) = result {
            panic!(
                "failed to parse {:?}: {:?} @ {} ({:?})",
                src,
                e,
                e.1,
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
            name: &'src str,
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
                let name = cols.next().unwrap();
                let domain = cols.next().unwrap();
                let path = cols.next().unwrap();
                let tag = cols.next().unwrap();
                let digest_algo = cols.next().unwrap();
                let digest_encoded = cols.next().unwrap();
                let err = cols.next().unwrap();
                Self {
                    input,
                    name,
                    domain: maybe(domain),
                    path: maybe(path),
                    tag: maybe(tag),
                    digest_algo: maybe(digest_algo),
                    digest_encoded: maybe(digest_encoded),
                    err: maybe(err),
                }
            }
        }
        fn as_test_case<'s>(span: &'s Reference<'s>) -> TestCase<'s> {
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
            let parsed = Reference::new(src);
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
