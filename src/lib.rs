//! # Parse docker/OCI Image References
//! This library is extensively tested against the authoritative image reference implementation,
//! <https://github.com/distribution/reference>.

#![no_std]
#![warn(missing_docs)]
#![warn(clippy::arithmetic_side_effects)]
#![warn(clippy::index_refutable_slice)]
#![warn(clippy::doc_markdown)]
#![warn(clippy::trivially_copy_pass_by_ref)] // TODO: consider inlining
#![deny(clippy::cast_possible_truncation)]
#![deny(clippy::cast_possible_wrap)]
#![deny(clippy::bad_bit_mask)]
#![warn(clippy::cast_enum_truncation)]
#![warn(clippy::checked_conversions)]
#![warn(clippy::copy_iterator)]
#![warn(clippy::deref_by_slicing)]
#![warn(clippy::cloned_instead_of_copied)]
#![warn(clippy::expect_used)]
#![warn(clippy::explicit_iter_loop)]
#![warn(clippy::get_unwrap)]
#![warn(clippy::invalid_upcast_comparisons)]
#![warn(clippy::missing_const_for_fn)]
#![warn(clippy::needless_borrow)]
#![warn(clippy::unwrap_used)]
#![warn(clippy::unwrap_in_result)]
#![warn(clippy::verbose_bit_mask)]
#![warn(clippy::try_err)]
#![warn(clippy::todo)]
#![warn(clippy::redundant_clone)]
// #![warn(clippy::indexing_slicing)] // used too often to enable
// #![warn(clippy::unreachable)]      // used too often to enable
// #![warn(clippy::or_fun_call)]      // warns about ok_or(Error::at(...))
pub(crate) mod ambiguous;
pub mod digest;
pub mod err;
pub mod name;
mod span;
pub mod tag;

#[doc(inline)]
pub use name::{domain, path};
use name::{domain::Domain, path::Path, Name, NameSpan};

use core::ops::{Range, RangeFrom};

use digest::Digest;

use crate::span::OptionallyZero;

use self::{
    ambiguous::domain_or_tagged_ref::DomainOrRefSpan, digest::DigestSpan, path::PathSpan,
    span::Lengthy, tag::TagSpan,
};
pub(crate) type Error = err::Error<u16>;
/// A reference to a container image. Must contain at least a name, but it may
/// also contain a tag and/or digest.
#[derive(PartialEq, Eq)]
struct RefSpan<'src> {
    /// the name of the image. This is the domain and path, but not the tag or digest.
    name: NameSpan<'src>,
    /// The image digest, if present
    tag: Option<TagSpan<'src>>,
    digest: Option<DigestSpan<'src>>,
}

impl<'src> RefSpan<'src> {
    fn new(src: &'src str) -> Result<Self, Error> {
        if src.is_empty() {
            return Error::at(0, err::Kind::RefMissing).into();
        };
        let prefix = DomainOrRefSpan::new(src)?;
        let domain = match prefix {
            DomainOrRefSpan::Domain(domain) => Some(domain),
            DomainOrRefSpan::TaggedRef(_) => None,
        };
        let mut index: u16 = domain.map(|d| d.short_len().upcast()).unwrap_or(0); // current max: 256
        let path = match src.as_bytes().get(prefix.len()) {
            Some(b'/') => match prefix {
                DomainOrRefSpan::TaggedRef((path_start, tag)) => match tag {
                    Some(_) => unreachable!(),
                    //         ^^^^^^^^^^^^ if a tag is present and is followed
                    //                      by a `/`, it's PortInvalidChar error
                    None => path_start.extend(&src[prefix.len()..]),
                    // e.g. "cant_be_host/more_path" needs to entirely match as path
                },
                DomainOrRefSpan::Domain(_) => {
                    index = index.saturating_add(1); // consume the leading slash; ok since index <= 256
                    let rest = &src[prefix.len() + 1..];
                    PathSpan::new(rest)
                }
            }
            .map_err(|e: err::Error<u8>| {
                Error::at(
                    prefix.short_len().upcast().saturating_add(e.index() as u16),
                    e.kind(),
                )
            }),
            Some(b'@') | Some(b':') | None => match prefix {
                DomainOrRefSpan::TaggedRef((name, _)) => Ok(name),
                DomainOrRefSpan::Domain(_) => unreachable!(),
                // ^ if the left segment peeked an '@', it would parse as a TaggedRef
            },
            Some(_) => Error::at(index, err::Kind::PathInvalidChar).into(),
        }?; // TODO: check correctness
        index = index.saturating_add(path.short_len().upcast().into()); // ok since index <= 256, path <= 256
        if index > name::MAX_LEN.into() {
            return Error::at(255, err::Kind::NameTooLong).into();
        }
        // let rest = &src[index as usize..];
        let tag = match prefix {
            DomainOrRefSpan::TaggedRef((_, right)) => match right {
                Some(tag) => Ok(Some(tag)),
                None => match src.as_bytes().get(index as usize) {
                    Some(b':') => TagSpan::new(&src[index as usize + 1..])
                        .map_err(|e| e.into())
                        .map(Some),
                    Some(b'@') | None => Ok(None),
                    Some(_) => Error::at(0, err::Kind::PathInvalidChar).into(),
                },
            },
            DomainOrRefSpan::Domain(_) => match src.as_bytes().get(index as usize) {
                Some(b':') => TagSpan::new(&src[index as usize + 1..])
                    .map(Some)
                    .map_err(|e| {
                        Error::at(
                            index
                        .saturating_add(1u16) // +1 to account for the leading ':'
                        .saturating_add(e.index().into()),
                            e.kind(),
                        )
                    }),
                Some(_) | None => Ok(None),
            },
        }?;
        index = index.saturating_add(
            // safe since tag <= 128ch and index <= 256ch -> max index = 384
            tag
                .map(|t: TagSpan| t.short_len().upcast().into())
                .map(|t: u16| t.saturating_add(1)) // +1 for the leading ':'
                .unwrap_or(0_u16),
        );
        let digest = match src.as_bytes().get(index as usize) {
            Some(b'@') => {
                index = index.saturating_add(1); // max 385
                DigestSpan::new(&src[index as usize..])
                    .map(Some)
                    .map_err(|e| Error::at(index.saturating_add(e.index()), e.kind()))
                // safe since e.index() <= 1024
            }
            Some(_) => Error::at(index, err::Kind::AlgorithmMissing).into(),
            None => Ok(None),
        }?;
        index = index.saturating_add(digest.map(|d| d.short_len().upcast()).unwrap_or(0));
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
        self.name.domain.map(|d| d.len() + 1).unwrap_or(0)
    }
    /// the at which the tag starts. If a tag is present, `tag_index` is AFTER the leading ':'.
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

    fn port_range(&self) -> Option<Range<usize>> {
        let domain = self.name.domain?;
        let port = domain.port?;
        let start = domain.host.len() + 1; // +1 to consume the ':'
        Some(start..start + port.len())
    }
    fn path_range(&self) -> Range<usize> {
        self.name
            .domain
            .map(|d| {
                let start = d.len() + 1; // +1 to consume the leading '/'
                start..(start + (self.name.path.len()))
            })
            .unwrap_or(0..self.name.path.len())
    }
    fn name_range(&self) -> Range<usize> {
        let end = self.path_index() + self.name.path.len();
        0..end
    }
    fn tag_range(&self) -> Option<Range<usize>> {
        let tag = self.tag?;
        let start = self.tag_index();
        Some(start..(start + tag.len()))
    }
    fn digest_range(&self) -> Option<RangeFrom<usize>> {
        self.digest.map(|_| self.digest_index()..)
    }
}

/// A reference to a container image. All references contain at least a name.
/// ```rust
/// use container_image_dist_ref::ImgRef;
/// let img_ref = ImgRef::new("host.com/repo:tag@algo:encoded").unwrap();
/// assert_eq!(img_ref.name().to_str(), "host.com/repo");
/// assert_eq!(img_ref.domain().map(|d| d.to_str()), Some("host.com"));
/// assert_eq!(img_ref.port(), None);
/// assert_eq!(img_ref.path().to_str(), "repo");
/// assert_eq!(img_ref.tag(), Some("tag"));
/// assert!(img_ref.digest().is_some());
/// let digest = img_ref.digest().unwrap();
/// assert_eq!(digest.to_str(), "algo:encoded");
/// ```
#[derive(PartialEq)]
pub struct ImgRef<'src> {
    src: &'src str,
    span: RefSpan<'src>,
}
impl<'src> ImgRef<'src> {
    /// Parse an image reference string. The entire source string must be one
    /// valid image reference.
    pub fn new(src: &'src str) -> Result<Self, Error> {
        let span = RefSpan::new(src)?;
        Ok(Self { src, span })
    }

    fn name_str(&self) -> &str {
        self.span.name.span_of(self.src)
    }
    /// Accessor for the name part of the reference, including the domain and path.
    pub fn name(&'src self) -> Name<'src> {
        Name::from_span(self.span.name, self.name_str())
    }
    #[allow(missing_docs)]
    pub fn domain(&'src self) -> Option<Domain<'src>> {
        self.span
            .name
            .domain
            .map(|d| Domain::from_span(d, d.span_of(self.src))) // TODO: unwrap_unsafe
    }
    /// The port part of the domain, if present. This does not include the leading `:`.
    pub fn port(&self) -> Option<&str> {
        self.span.port_range().and_then(|r| self.src.get(r))
    }
    fn path_str(&self) -> &str {
        let range = self.span.path_range();
        &self.src[range]
    }
    /// the path portion of the reference, NOT including any leading `/` if a domain is present.
    pub fn path(&'src self) -> Path<'src> {
        Path::from_span(self.span.name.path, self.path_str())
    }
    /// Accessor the tag part of the reference NOT including the leading `:`
    pub fn tag(&self) -> Option<&str> {
        self.span.tag_range().and_then(|range| {
            debug_assert!(self.src.get(range.clone()).is_some());
            self.src.get(range)
        })
    }
    /// Accessor for the optional digest part of the reference NOT including the leading `@`
    pub fn digest(&self) -> Option<Digest<'src>> {
        self.span.digest.and_then(|digest_span| {
            debug_assert!(self.src.get(self.span.digest_index()..).is_some());
            Some(Digest::from_span(
                self.src.get(self.span.digest_index()..)?,
                digest_span,
            ))
        })
    }
}

/// produce an u8 representing the amount of information contained in the span
/// higher = more information, lower = less information
fn rank(span: &RefSpan) -> u8 {
    span.name.domain.map(|_| 1 << 2).unwrap_or(0)
        | span.tag.map(|_| 1 << 1).unwrap_or(0)
        | span.digest.map(|_| 1 << 0).unwrap_or(0)
}
impl PartialOrd for RefSpan<'_> {
    fn partial_cmp(&self, other: &Self) -> Option<core::cmp::Ordering> {
        rank(other).partial_cmp(&rank(self))
        // note the order: if other has more information than self, then other
        // has to be ordered before than self
    }
}

impl PartialOrd for ImgRef<'_> {
    fn partial_cmp(&self, other: &Self) -> Option<core::cmp::Ordering> {
        self.span.partial_cmp(&other.span)
    }
}

struct CanonicalSpan<'src> {
    span: RefSpan<'src>,
}
macro_rules! mirror_inner_method {
    ($inner:ident, $res:ty) => {
        #[inline]
        fn $inner(&self) -> $res {
            self.span.$inner()
        }
    };
}
macro_rules! unwrap_inner_method {
    ($inner:ident, $res:ty) => {
        #[inline]
        fn $inner(&self) -> $res {
            self.span.$inner().unwrap()
        }
    };
}

impl<'src> CanonicalSpan<'src> {
    fn new(src: &'src str) -> Result<Self, Error> {
        Self::from_span(RefSpan::new(src)?)
    }
    // FIXME: move from_span -> TryFrom<RefSpan> impl
    fn from_span(span: RefSpan<'src>) -> Result<Self, Error> {
        span.name
            .domain
            .ok_or(Error::at(0, err::Kind::HostMissing))?;
        #[allow(clippy::cast_possible_truncation)]
        span.digest.ok_or(Error::at(
            span.digest_index() as u16, // safe since digest_index must be less than  u16::MAX
            err::Kind::AlgorithmMissing,
        ))?;
        Ok(Self { span })
    }
    mirror_inner_method!(path_range, Range<usize>);
    mirror_inner_method!(name_range, Range<usize>);
    mirror_inner_method!(tag_range, Option<Range<usize>>);
    unwrap_inner_method!(digest_range, RangeFrom<usize>);
}

/// A canonical image reference includes a domain, a path/repo name, and digest.
/// It may also include a tag.
/// ```rust
/// use container_image_dist_ref::CanonicalImgRef;
/// let img_ref = CanonicalImgRef::new("host.com/repo:tag@algo:encoded").unwrap();
/// assert_eq!(img_ref.name().to_str(), "host.com/repo");
/// assert_eq!(img_ref.domain().to_str(), "host.com");
/// assert_eq!(img_ref.path().to_str(), "repo");
/// assert_eq!(img_ref.tag(), Some("tag"));
/// assert_eq!(img_ref.digest().to_str(), "algo:encoded");
///
/// let img_ref = CanonicalImgRef::new("no.tag/img@algo:encoded").unwrap();
/// assert_eq!(img_ref.name().to_str(), "no.tag/img");
/// assert_eq!(img_ref.domain().to_str(), "no.tag");
/// assert_eq!(img_ref.path().to_str(), "img");
/// assert_eq!(img_ref.tag(), None);
/// assert_eq!(img_ref.digest().to_str(), "algo:encoded");
/// ```
pub struct CanonicalImgRef<'src> {
    src: &'src str,
    span: CanonicalSpan<'src>,
}
impl<'src> CanonicalImgRef<'src> {
    /// Parse a canonical image reference string. The entire source string must be one
    /// valid canonical reference.
    pub fn new(src: &'src str) -> Result<Self, Error> {
        let span = CanonicalSpan::new(src)?;
        Ok(Self { src, span })
    }
    fn domain_str(&self) -> &str {
        self.span
            .span
            .name
            .domain
            .and_then(|d| {
                debug_assert!(self.src.get(0..d.len()).is_some());
                self.src.get(0..d.len())
            })
            .unwrap_or("")
    }
    fn path_str(&self) -> &str {
        let path = &self.src[self.span.path_range()];
        debug_assert!(
            !path.is_empty(),
            "canonical refs should have non-empty paths by construction"
        );
        path
    }
    fn name_str(&self) -> &str {
        let result = &self.src[self.span.name_range()];
        debug_assert!(
            !result.is_empty(),
            "canonical refs should have non-empty names by construction"
        );
        result
    }
    /// The name component of the parse canonical image reference, including a
    /// required domain and path.
    pub fn name(&'src self) -> Name<'src> {
        Name::from_span(self.span.span.name, self.name_str())
    }
    /// The domain component of the canonical image reference.
    #[allow(clippy::unwrap_used)]
    pub fn domain(&'src self) -> Domain<'src> {
        Domain::from_span(self.span.span.name.domain.unwrap(), self.domain_str())
    }
    /// The path component of the canonical image reference.
    pub fn path(&'src self) -> Path<'src> {
        Path::from_span(self.span.span.name.path, self.path_str())
    }
    /// The tag component of the canonical image reference, if present.
    pub fn tag(&self) -> Option<&str> {
        // tags aren't required for canonical refs
        self.span.tag_range().and_then(|range| {
            debug_assert!(self.src.get(range.clone()).is_some());
            self.src.get(range)
        })
    }
    #[allow(clippy::unwrap_used)]
    /// The digest component of the canonical image reference.
    pub fn digest(&self) -> Digest<'src> {
        let digest = &self.src[self.span.digest_range()];
        debug_assert!(
            !digest.is_empty(),
            "canonical refs should have non-empty digests by construction"
        );
        Digest::from_span(digest, self.span.span.digest.unwrap())
    }
}

impl<'src> From<CanonicalImgRef<'src>> for ImgRef<'src> {
    fn from(value: CanonicalImgRef<'src>) -> Self {
        Self {
            src: value.src,
            span: value.span.span,
        }
    }
}
impl<'src> TryInto<CanonicalSpan<'src>> for RefSpan<'src> {
    type Error = Error;
    fn try_into(self) -> Result<CanonicalSpan<'src>, self::Error> {
        CanonicalSpan::from_span(self)
    }
}
impl<'src> TryInto<CanonicalImgRef<'src>> for ImgRef<'src> {
    type Error = Error;
    fn try_into(self) -> Result<CanonicalImgRef<'src>, Self::Error> {
        let canonical = self.span.try_into()?;
        Ok(CanonicalImgRef {
            src: self.src,
            span: canonical,
        })
    }
}
#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {

    extern crate alloc;

    use alloc::{format, string::String};

    use self::err::Error;

    use super::*;
    fn should_parse(src: &'_ str) -> ImgRef<'_> {
        let result = ImgRef::new(src);
        if let Err(e) = result {
            panic!("{}", pretty_err(e, src));
        }
        result.unwrap()
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
    fn should_parse_as(
        src: &str,
        domain: Option<&str>,
        path: Option<&str>,
        tag: Option<&str>,
        digest: Option<&str>,
    ) {
        let actual = should_parse(src);
        let actual_domain = actual.domain().map(|d| d.to_str());
        let actual_path = actual.path();
        let actual_path = actual_path.to_str();
        let actual_tag = actual.tag();
        let actual_digest = actual.digest().map(|d| d.to_str());
        let actual = (actual_domain, Some(actual_path), actual_tag, actual_digest);
        let expected = (domain, path, tag, digest);
        assert_eq!(actual, expected, "differences parsing {:?}", src);
    }

    fn should_fail_with(src: &'_ str, expected: Error<u16>) {
        let result = ImgRef::new(src);

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
            src.push_str(tag);
            should_parse_as(&src, None, Some("0"), Some(tag), None);
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
            src.push('0');
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
            src.push('0');
            src.push('@');
            src.push_str(&"0".repeat(128));
            src.push('+');
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
            src.push(':');
            should_fail_with(&src, Error::at(258, err::Kind::EncodedMissing))
        };
    }
    #[test]
    fn test_bad_ipv6_fails() {
        should_fail_with("[::]0", Error::at(4, err::Kind::PortOrTagInvalidChar));
    }

    #[test]
    fn test_canonical() {
        let canonical = CanonicalImgRef::new("[2001:db8::1]:5000/repo@algo:encoded").unwrap();
        assert_eq!(canonical.name_str(), "[2001:db8::1]:5000/repo");
        assert_eq!(canonical.domain_str(), "[2001:db8::1]:5000");
        assert_eq!(canonical.path_str(), "repo");
        assert_eq!(canonical.tag(), None);
        assert_eq!(canonical.digest().to_str(), "algo:encoded");
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
            const fn maybe(s: &str) -> Option<&str> {
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
    impl TestCase<'_> {
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
    fn as_test_case<'s>(span: &'s ImgRef<'s>) -> TestCase<'s> {
        TestCase {
            input: span.src,
            name: Some(span.name_str()),
            domain: span.domain().map(|d| d.to_str()),
            path: Some(span.path().to_str()),
            tag: span.tag(),
            digest_algo: span.digest().map(|d| d.algorithm().to_str()),
            digest_encoded: span.digest().map(|d| d.encoded().to_str()),
            err: None,
        }
    }
    #[test]
    fn basic_corpus() {
        fn expect(src: &str, expected: TestCase) {
            let parsed = ImgRef::new(src);
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
            .map(TestCase::from);
        valid_inputs
            .chain(invalid_inputs)
            .zip(expected_outputs)
            .for_each(|(src, expected)| expect(src, expected))
    }
}
