// TODO: docstring

#![no_std]
pub(crate) mod ambiguous;
pub mod digest;
pub mod err;
pub mod name;
mod span;
pub mod tag;

use name::NameSpan;
pub use name::{domain, path};

use core::ops::{Range, RangeFrom};

use digest::DigestStr;

use crate::span::OptionallyZero;

use self::{
    ambiguous::domain_or_tagged_ref::DomainOrRefSpan, digest::DigestSpan, domain::DomainSpan,
    path::PathSpan, span::Lengthy, tag::TagSpan,
};
pub(crate) type Error = err::Error<u16>;
/// A reference to an image by any combination of name, tag, and digest.
// TODO: doctest
#[derive(PartialEq, Eq)]
struct RefSpan<'src> {
    name: NameSpan<'src>,
    tag: Option<TagSpan<'src>>,
    digest: Option<DigestSpan<'src>>,
}

impl<'src> RefSpan<'src> {
    pub fn new(src: &'src str) -> Result<Self, Error> {
        if src.is_empty() {
            return Error::at(0, err::Kind::RefMissing).into();
        };
        let prefix = DomainOrRefSpan::new(src)?;
        let rest = &src[prefix.len()..];
        let domain = match prefix {
            DomainOrRefSpan::Domain(domain) => Some(domain),
            DomainOrRefSpan::TaggedRef(_) => None,
        };
        let mut index: u16 = domain.map(|d| d.short_len().into()).unwrap_or(0);
        let path = match rest.bytes().next() {
            Some(b'/') => match prefix {
                DomainOrRefSpan::TaggedRef((path_start, tag)) => match tag {
                    Some(_) => {
                        unreachable!("a:0/b should always be parsed as Domain(host=a, port=0)")
                    }
                    None => path_start.extend(rest).map_err(|e| e.into()),
                    // e.g. "cant_be_host/more_path" needs to entirely match as path
                },
                DomainOrRefSpan::Domain(_) => PathSpan::parse_from_slash(rest)
                    .and_then(|p| p.ok_or(err::Error::<u8>::at(0, err::Kind::PathMissing)))
                    .map_err(|e| e.into()),
            }
            .map_err(|e: err::Error<u16>| e + prefix.short_len()),
            Some(b'@') | Some(b':') | None => match prefix {
                DomainOrRefSpan::TaggedRef((name, _)) => Ok(name),
                DomainOrRefSpan::Domain(_) => {
                    unreachable!("if the left segment peeked an '@', it would parse as a TaggedRef")
                }
            },
            Some(_) => Error::at(0, err::Kind::PathInvalidChar).into(),
        }
        .map_err(|e| e + index)?;
        index += path.short_len().upcast() as u16;
        let path = path;
        if index > name::MAX_LEN.into() {
            return Error::at(index, err::Kind::NameTooLong).into();
        }
        let rest = &src[index as usize..];
        let tag = match prefix {
            DomainOrRefSpan::TaggedRef((_, right)) => match right {
                Some(tag) => Ok(Some(tag)),
                None => match rest.bytes().next() {
                    Some(b':') => TagSpan::new(&rest[1..]).map_err(|e| e.into()),
                    Some(b'@') | None => Ok(None),
                    Some(_) => Error::at(0, err::Kind::PathInvalidChar).into(),
                },
            },
            DomainOrRefSpan::Domain(_) => match rest.bytes().next() {
                Some(b':') => TagSpan::new(&rest[1..])
                    .map_err(|e| e.into())
                    .map_err(|e: err::Error<u16>| e + 1u16), // +1 to account for the leading ':'
                Some(_) | None => Ok(None),
            },
        }
        .map_err(|e| e + index)?;
        index += tag
            .map(|t| t.short_len().upcast().into())
            .map(|l: u16| l + 1) // +1 for the leading ':'
            .unwrap_or(0);
        let rest = &src[index as usize..];
        let digest = match rest.bytes().next() {
            Some(b'@') => {
                index += 1;
                DigestSpan::new(&src[index as usize..])
            }
            Some(_) => Error::at(index, err::Kind::AlgorithmMissing).into(),
            None => Ok(None),
        }
        .map_err(|e| e + index)?;
        index += digest.map(|d| d.short_len().upcast()).unwrap_or(0);
        debug_assert!(
            index as usize == src.len(),
            "index {} != src.len() {}",
            index,
            src.len()
        );
        Ok(Self {
            name: NameSpan { domain, path },
            tag,
            digest,
        })
    }
    /// the offset at which the path starts.
    fn path_index(&self) -> usize {
        self.name.domain.map(|d| d.len()).unwrap_or(0)
    }
    /// the at which the tag starts. If a tag is present, tag_index is AFTER the leading ':'.
    fn tag_index(&self) -> usize {
        self.path_index()
            + self.name.path.len()
            + self.tag.map(|_| 1) // +1 for the leading ':'
            .unwrap_or(0)
    }
    fn digest_index(&self) -> usize {
        self.tag_index() // tag_index() accounts for the leading ':'
            + self.tag.map(|t| t.len()).unwrap_or(0)
            + self.digest.map(|_| 1) // 1 == consume the leading '@' if a digest is present
            .unwrap_or(0)
    }

    fn domain_range(&self) -> Option<Range<usize>> {
        self.name.domain.map(|d| 0..d.len())
    }
    fn path_range(&self) -> Range<usize> {
        let mut start = self.path_index();
        let end = start + self.name.path.len();
        if self.name.domain.is_some() {
            // don't emit the leading '/'
            start += 1;
        }
        start..end
    }
    fn name_range(&self) -> Range<usize> {
        let end = self.path_index() + self.name.path.len();
        0..end
    }
    fn tag_range(&self) -> Option<Range<usize>> {
        self.tag.map(|t| {
            let start = self.tag_index();
            start..(start + t.len())
        })
    }
    fn digest_range(&self) -> Option<RangeFrom<usize>> {
        self.digest.map(|_| self.digest_index()..)
    }
}

/// A *canonical* image reference includes:
/// - A domain
/// - a path/repo name
/// - optionally, a tag
/// - A digest
// TODO: doctest
pub struct CanonicalSpan<'src> {
    domain: DomainSpan<'src>,
    path: PathSpan<'src>,
    tag: Option<TagSpan<'src>>,
    digest: DigestSpan<'src>,
}
impl<'src> CanonicalSpan<'src> {
    pub fn new(src: &'src str) -> Result<Self, Error> {
        let domain = DomainSpan::new(src)?.ok_or(Error::at(0, err::Kind::HostMissing))?;
        let mut len = domain.short_len().into();
        match &src[len as usize..].bytes().next() {
            Some(b'/') => Ok(()),
            Some(_) => Err(err::Kind::PathInvalidChar),
            None => Err(err::Kind::PathMissing),
        }
        .map_err(|kind| Error::at(len, kind))?;

        let path = PathSpan::new(&src[len as usize..])
            .map_err(|e| e.into())
            .map_err(|e: Error| e + len)?
            .ok_or(Error::at(len, err::Kind::PathMissing))?;
        len += path.short_len().upcast() as u16;
        if len > name::MAX_LEN as u16 {
            return Error::at(len, err::Kind::NameTooLong).into();
        }
        let tag = TagSpan::new(&src[len as usize..]) // can be None
            .map_err(|e| e.into())
            .map_err(|e: Error| e + len)?;
        len += tag.map(|t| t.short_len().upcast().into()).unwrap_or(0);
        len += match src.as_bytes()[len as usize..].iter().next() {
            Some(b'@') => Ok(1),
            Some(_) => Error::at(len, err::Kind::PathInvalidChar).into(),
            None => Error::at(len, err::Kind::AlgorithmMissing).into(),
        }?;
        let digest = DigestSpan::new(&src[len as usize..])
            .map_err(|e| e + len)?
            .ok_or(Error::at(len, err::Kind::AlgorithmMissing))?;
        Ok(Self {
            domain,
            path,
            tag,
            digest,
        })
    }
}

// TODO: add docs with doctest examples
#[derive(PartialEq)]
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
    pub fn path(&self) -> &str {
        &self.src[self.span.path_range()]
    }
    pub fn name(&self) -> &str {
        &self.src[self.span.name_range()]
    }
    pub fn tag(&self) -> Option<&str> {
        self.span.tag_range().map(|range| &self.src[range])
    }
    pub fn digest(&self) -> Option<DigestStr<'src>> {
        self.span.digest_range().and_then(|range| {
            self.span
                .digest
                .map(|span| DigestStr::from_span(&self.src[range], span))
        })
    }
}

/// produce an u8 representing the amount of information contained in the span
/// higher = more information, lower = less information
fn rank(span: &RefSpan) -> u8 {
    span.name.domain.map(|_| 1 << 3).unwrap_or(0)
        | span.tag.map(|_| 1 << 1).unwrap_or(0)
        | span.digest.map(|_| 1 << 0).unwrap_or(0)
}
impl<'src> PartialOrd for RefSpan<'src> {
    fn partial_cmp(&self, other: &Self) -> Option<core::cmp::Ordering> {
        rank(other).partial_cmp(&rank(self))
        // note the order: if other has more information than self, then other
        // has to be ordered before than self
    }
}

impl<'src> PartialOrd for RefStr<'src> {
    fn partial_cmp(&self, other: &Self) -> Option<core::cmp::Ordering> {
        self.span.partial_cmp(&other.span)
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
        let domain = self.span.domain.span_of(self.src);
        debug_assert!(
            !domain.is_empty(),
            "canonical refs should have non-empty domains by construction"
        );
        domain
    }
    pub fn path(&self) -> &str {
        let path = self
            .span
            .path
            .span_of(&self.src[self.span.domain.len() + 1..]);
        // +1 for the '/' between the domain and path
        debug_assert!(
            !path.is_empty(),
            "canonical refs should have non-empty paths by construction"
        );
        path
    }
    pub fn name(&self) -> &str {
        let result = &self.src[0..self.domain().len() + 1 + self.path().len()];
        // +1 for the '/' between the domain and path
        debug_assert!(
            !result.is_empty(),
            "canonical refs should have non-empty names by construction"
        );
        result
    }
    pub fn tag(&self) -> Option<&str> {
        // tags aren't required for canonical refs
        self.span.tag.map(|t| {
            let start = self.span.domain.len() + 1 + self.path().len();
            let end = start + t.len();
            &self.src[(start + 1)..end] // trim the leading ':'
        })
    }
    pub fn digest(&self) -> DigestStr<'src> {
        let start = self.span.domain.len()
            + 1 // 1 == '/'
            + self.path().len()
            + self.span.tag.map(|t| 1 + t.len()) // 1 == ':'
            .unwrap_or(0);
        let src = &self.src[start..];
        debug_assert!(
            !src.is_empty(),
            "canonical refs should have non-empty digests by construction"
        );
        DigestStr::from_span(src, self.span.digest)
    }
}

impl<'src> From<CanonicalStr<'src>> for RefStr<'src> {
    fn from(value: CanonicalStr<'src>) -> Self {
        Self {
            src: value.src,
            span: RefSpan {
                name: NameSpan {
                    domain: Some(value.span.domain),
                    path: value.span.path,
                },
                tag: value.span.tag,
                digest: Some(value.span.digest),
            },
        }
    }
}
impl<'src> TryInto<CanonicalSpan<'src>> for RefSpan<'src> {
    type Error = Error;
    fn try_into(self) -> Result<CanonicalSpan<'src>, self::Error> {
        // a canonical reference needs a domain and digest
        let domain = self
            .name
            .domain
            .ok_or(Error::at(0, err::Kind::HostMissing))?;
        if self.digest.is_none() {
            return Error::at(
                self.digest_index().try_into().unwrap(),
                err::Kind::AlgorithmMissing,
            )
            .into();
        }
        let digest = self.digest.ok_or(Error::at(
            self.digest_index().try_into().unwrap(), // safe to unwrap since host + path + tag + algorithm MUST be under u16::MAX
            err::Kind::AlgorithmMissing,             // TODO: more specific error?
        ))?;

        Ok(CanonicalSpan {
            domain,
            path: self.name.path,
            tag: self.tag,
            digest,
        })
    }
}
impl<'src> TryInto<CanonicalStr<'src>> for RefStr<'src> {
    type Error = Error;
    fn try_into(self) -> Result<CanonicalStr<'src>, Self::Error> {
        let canonical = self.span.try_into()?;
        Ok(CanonicalStr {
            src: self.src,
            span: canonical,
        })
    }
}
#[cfg(test)]
mod tests {

    extern crate alloc;

    use alloc::{format, string::String};

    use self::err::Error;

    use super::*;
    fn should_parse(src: &'_ str) -> RefStr<'_> {
        let result = RefStr::new(src);
        if let Err(e) = result {
            panic!("{}", pretty_err(e, src));
        }
        let span = result.unwrap();
        span
    }
    // TODO: expose this kind of error-formatting functionality in the err module
    // behind an `alloc` feature flag
    fn pretty_err(e: Error<u16>, src: &str) -> String {
        let kind = e.kind();
        let index = e.index();
        let msg = "failed to parse";
        let padding = " ".repeat(msg.len() + 2 + index as usize);
        format!("{msg} \"{src}\": {kind:?} @ {index}\n{padding}^")
    }
    fn should_parse_as<'src>(
        src: &'src str,
        domain: Option<&str>,
        path: Option<&str>,
        tag: Option<&str>,
        digest: Option<&str>,
    ) {
        let span = should_parse(src);
        let actual = (
            span.domain(),
            Some(span.path()),
            span.tag(),
            span.digest().map(|d| d.src()),
        );
        let expected = (domain, path, tag, digest);
        assert_eq!(actual, expected, "differences parsing {:?}", src);
    }

    fn should_fail_with(src: &'_ str, expected: Error<u16>) {
        let result = RefStr::new(src);

        match result {
            Ok(r) => panic!(
                "expected parsing {src:?} to fail, but it succeeded as {:#?}",
                as_test_case(&r)
            ),
            Err(e) => {
                assert_eq!(
                    e.index(),
                    expected.index(),
                    "expected {expected:?}, got {e:?} when parsing {src:?}",
                );
                assert_eq!(
                    e.kind(),
                    expected.kind(),
                    "expected:\n{}\ngot:\n{}",
                    pretty_err(expected, src),
                    pretty_err(e, src),
                );
            }
        }
    }
    #[test]
    fn test_name_only() {
        should_parse_as("test_com", None, Some("test_com"), None, None);
        should_parse_as("test.com", None, Some("test.com"), None, None);
        let s = "0".repeat(255);
        should_parse_as(&s, None, Some(&s), None, None);
    }
    #[test]
    fn test_tagged_ref() {
        should_parse_as("0_0/0:0", None, Some("0_0/0"), Some("0"), None);
        should_parse_as("test.com:tag", None, Some("test.com"), Some("tag"), None);
        should_parse_as("test.com:5000", None, Some("test.com"), Some("5000"), None);
        should_parse_as("0:0A", None, Some("0"), Some("0A"), None);
        should_parse_as("0:_", None, Some("0"), Some("_"), None);

        should_fail_with(
            "bad:port/path:tag",
            err::Error::at(4, err::Kind::PortInvalidChar),
        );
        {
            let mut src = String::with_capacity(130);
            src.push_str("0:");
            let tag = &"0".repeat(128); // max tag length
            src.push_str(&tag);
            should_parse_as(&src, None, Some("0"), Some(&tag), None);
        }
        {
            let mut src = String::with_capacity(u8::MAX as usize * 2 + 1);
            let long = "0".repeat(u8::MAX as usize);
            src.push_str(&long);
            src.push(':');
            src.push_str(&long);
            should_fail_with(
                &src,
                Error::at(u8::MAX as u16 + 1 + 128, err::Kind::TagTooLong),
                // Short::MAX == max name length
                //          1 == the ':' of the tag
                //        128 == max length of tag characters
            );
        }
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
        );
        should_parse_as("0_0/0", None, Some("0_0/0"), None, None);
        should_fail_with("0_0/", Error::at(4, err::Kind::PathComponentInvalidEnd));
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
        {
            let mut src = String::with_capacity(2 + 256);
            src.push_str("0");
            src.push('@');
            src.push_str(&"0".repeat(256)); // too long
            let too_long = 1 // '0', the name
                + 1 // '@'
                + 255 // max allowed algorithm length
                ;
            should_fail_with(&src, Error::at(too_long, err::Kind::AlgorithmTooLong));
        };
        {
            let mut src = String::with_capacity(2 + 257);
            src.push_str("0");
            src.push('@');
            src.push_str(&"0".repeat(128));
            src.push_str("+");
            src.push_str(&"0".repeat(128)); // too long!
            let too_long = 1 // '0', the name
                + 1 // '@'
                + 255 // max allowed algorithm length
                ;
            should_fail_with(&src, Error::at(too_long, err::Kind::AlgorithmTooLong));
        };
        {
            let mut src = String::with_capacity(2 + 256);
            src.push_str("0@");
            src.push_str(&"0".repeat(255));
            src.push_str(":");
            should_fail_with(&src, Error::at(258, err::Kind::EncodedMissing))
        };
    }
    #[test]
    fn test_bad_ipv6_fails() {
        should_fail_with("[::]0", Error::at(4, err::Kind::PortOrTagInvalidChar));
    }
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
    impl<'src> TestCase<'src> {
        fn diff(&self, other: &Self) -> Result<(), String> {
            if self == other {
                Ok(())
            } else {
                let mut diff = String::from("--- expected\n+++ actual\n");
                debug_assert!(self.input == other.input);
                diff.push_str(format!("  input: {}\n", self.input).as_str());
                if self.name == other.name {
                    diff.push_str(format!("  name: {:?}\n", self.name).as_str());
                } else {
                    diff.push_str(format!("- name: {:?}\n", self.name).as_str());
                    diff.push_str(format!("+ name: {:?}\n", other.name).as_str());
                }
                if self.domain == other.domain {
                    diff.push_str(format!("  domain: {:?}\n", self.domain).as_str());
                } else {
                    diff.push_str(format!("- domain: {:?}\n", self.domain).as_str());
                    diff.push_str(format!("+ domain: {:?}\n", other.domain).as_str());
                }
                if self.path == other.path {
                    diff.push_str(format!("  path: {:?}\n", self.path).as_str());
                } else {
                    diff.push_str(format!("- path: {:?}\n", self.path).as_str());
                    diff.push_str(format!("+ path: {:?}\n", other.path).as_str());
                }
                if self.tag == other.tag {
                    diff.push_str(format!("  tag: {:?}\n", self.tag).as_str());
                } else {
                    diff.push_str(format!("- tag: {:?}\n", self.tag).as_str());
                    diff.push_str(format!("+ tag: {:?}\n", other.tag).as_str());
                }
                if self.digest_algo == other.digest_algo {
                    diff.push_str(format!("  digest_algo: {:?}\n", self.digest_algo).as_str());
                } else {
                    diff.push_str(format!("- digest_algo: {:?}\n", self.digest_algo).as_str());
                    diff.push_str(format!("+ digest_algo: {:?}\n", other.digest_algo).as_str());
                }
                if self.digest_encoded == other.digest_encoded {
                    diff.push_str(
                        format!("  digest_encoded: {:?}\n", self.digest_encoded).as_str(),
                    );
                } else {
                    diff.push_str(
                        format!("- digest_encoded: {:?}\n", self.digest_encoded).as_str(),
                    );
                    diff.push_str(
                        format!("+ digest_encoded: {:?}\n", other.digest_encoded).as_str(),
                    );
                }
                Err(diff)
            }
        }
    }
    fn as_test_case<'s>(span: &'s RefStr<'s>) -> TestCase<'s> {
        let digest = span.digest();
        TestCase {
            input: span.src,
            name: Some(span.name()),
            domain: span.domain(),
            path: Some(span.path()),
            tag: span.tag(),
            digest_algo: digest.as_ref().map(|d| d.algorithm().src()),
            digest_encoded: digest.map(|d| d.encoded().src()),
            err: None,
        }
    }
    #[test]
    fn basic_corpus() {
        fn expect(src: &str, expected: TestCase) {
            let parsed = RefStr::new(src);
            match (expected.err, parsed) {
                (Some(_err), Err(_e)) => {} // ok
                (None, Ok(actual)) => {
                    let actual = as_test_case(&actual);
                    match expected.diff(&actual) {
                        Ok(_) => {} // ok
                        Err(diff) => panic!("{diff}"),
                    }
                }
                (Some(err), Ok(_span)) => {
                    panic!("expected {src:?} to fail with {err:?}, but it succeeded")
                }
                (None, Err(e)) => panic!(
                    "expected {src:?} to succeed, but it failed with:\n{}",
                    pretty_err(e, src)
                ),
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
