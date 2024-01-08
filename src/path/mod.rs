//! The current RC of the OCI distribution spec and distribution/reference agree
//! on the format of a path component. The OCI distribution spec's current RC is
//! a strict superset of the previous path pattern, so implementing the newer, more-
//! compatible pattern shouldn't result in any previously-valid paths being rejected.
//!
//! > ```bnf
//! >    path (or "remote-name")  := path-component ['/' path-component]*
//! >    path-component           := alpha-numeric [separator alpha-numeric]*
//! >    alpha-numeric            := /[a-z0-9]+/
//! >    separator                := /[_.]|__|[-]*/
//! > ```
//! > -- https://github.com/distribution/reference/blob/v0.5.0/reference.go#L7-L16
//!
//!
//!
//! > Throughout this document, <name> MUST match the following regular expression:
//! > ```ebnf
//! > [a-z0-9]+([._-][a-z0-9]+)*(/[a-z0-9]+([._-][a-z0-9]+)*)*
//! > [a-z0-9]+((\.|_|__|-+)[a-z0-9]+)*(\/[a-z0-9]+((\.|_|__|-+)[a-z0-9]+)*)*
//! > ```
//! > -- https://github.com/opencontainers/distribution-spec/blob/v1.0.1/spec.md#pulling-manifests
//! > -- https://github.com/opencontainers/distribution-spec/blob/v1.1.0-rc3/spec.md#pulling-manifests
//! > -- https://github.com/opencontainers/distribution-spec/commit/a73835700327bd1c037e33d0834c46ff98ac1286
//! > -- https://github.com/opencontainers/distribution-spec/commit/efe2de09470d7f182d2fbd83ac4462fbdc462455

use crate::{
    ambiguous::host_or_path::{Error as HostOrPathError, Kind as PathKind, OptionalHostOrPath},
    err::{Error, Kind as ErrorKind},
    span::{impl_span_methods_on_tuple, IntoOption, OptionalSpan, U},
};

pub(super) enum EKind {
    NoMatch,
    ComponentInvalidEnd,
    InvalidChar,
    TooLong,

    Ipv6NoMatch,
    Ipv6TooLong,
    Ipv6BadColon,
    Ipv6TooManyHexDigits,
    Ipv6TooManyGroups,
    Ipv6TooFewGroups,
    Ipv6MissingClosingBracket,
}
pub(super) struct _Error(EKind, U);
impl From<HostOrPathError> for _Error {
    fn from(err: HostOrPathError) -> Self {
        use crate::ambiguous::host_or_path::AmbiguousErrorKind as A;
        match err.kind() {
            A::NoMatch => _Error(EKind::NoMatch, err.len()),
            A::TooLong => _Error(EKind::TooLong, err.len()),
            A::InvalidChar => _Error(EKind::InvalidChar, err.len()),
            _ => unreachable!("ipv6 errors should never be raised in this call path"),
            // A::Ipv6NoMatch => _Error(EKind::Ipv6NoMatch, err.len()),
            // A::Ipv6TooLong => _Error(EKind::Ipv6TooLong, err.len()),
            // A::Ipv6BadColon => _Error(EKind::Ipv6BadColon, err.len()),
            // A::Ipv6TooManyHexDigits => _Error(EKind::Ipv6TooManyHexDigits, err.len()),
            // A::Ipv6TooManyGroups => _Error(EKind::Ipv6TooManyGroups, err.len()),
            // A::Ipv6TooFewGroups => _Error(EKind::Ipv6TooFewGroups, err.len()),
            // A::Ipv6MissingClosingBracket => _Error(EKind::Ipv6MissingClosingBracket, err.len()),
        }
    }
}
impl From<_Error> for Error {
    fn from(err: _Error) -> Error {
        use EKind as Src;
        use ErrorKind as Dest;
        let (kind, len) = (err.0, err.1);
        let kind = match kind {
            Src::NoMatch => Dest::PathNoMatch,
            Src::ComponentInvalidEnd => Dest::PathComponentInvalidEnd,
            Src::InvalidChar => Dest::PathInvalidChar,
            Src::TooLong => Dest::PathTooLong,
            Src::Ipv6NoMatch => Dest::Ipv6NoMatch,
            Src::Ipv6TooLong => Dest::Ipv6TooLong,
            Src::Ipv6BadColon => Dest::Ipv6BadColon,
            Src::Ipv6TooManyHexDigits => Dest::Ipv6TooManyHexDigits,
            Src::Ipv6TooManyGroups => Dest::Ipv6TooManyGroups,
            Src::Ipv6TooFewGroups => Dest::Ipv6TooFewGroups,
            Src::Ipv6MissingClosingBracket => Dest::Ipv6MissingClosingBracket,
        };
        Error(kind, len)
    }
}

impl core::ops::Add<U> for _Error {
    type Output = Self;
    fn add(self, rhs: U) -> Self {
        Self(self.0, self.1 + rhs)
    }
}

#[derive(Clone, Copy)]
pub(crate) struct PathSpan<'src>(OptionalSpan<'src>);
impl_span_methods_on_tuple!(PathSpan);

impl<'src> TryFrom<OptionalHostOrPath<'src>> for PathSpan<'src> {
    type Error = Error;
    fn try_from(ambiguous: OptionalHostOrPath) -> Result<Self, Error> {
        use PathKind::*;
        match ambiguous.kind() {
            Either | Path => Ok(if ambiguous.is_some() {
                Self(OptionalSpan::new(ambiguous.short_len()))
            } else {
                Self::none()
            }),
            Host => Err(Error(ErrorKind::PathInvalidChar, ambiguous.short_len())),
            // FIXME: find the underscore(s) in the path ^^^^
            IpV6 => Ok(Self(OptionalSpan::new(ambiguous.short_len()))),
        }
    }
}

impl<'src> PathSpan<'src> {
    fn none() -> Self {
        Self(OptionalSpan::new(0))
    }
    pub(crate) fn new(src: &'src str) -> Result<Self, Error> {
        let mut len: usize = 0;
        loop {
            let section = OptionalHostOrPath::new(&src[len..], PathKind::Path)
                .map_err(|e| Into::<_Error>::into(e))?;
            len += section.len();
            if src[len..].bytes().next() == Some(b'/') {
                len += 1;
                continue;
            } else {
                break;
            }
        }
        Ok(Self(OptionalSpan::new(len.try_into().unwrap())))
    }
}

pub struct PathStr<'src>(&'src str);
impl<'src> PathStr<'src> {
    pub(crate) fn src(&self) -> &'src str {
        self.0
    }
    #[inline(always)]
    pub fn len(&self) -> usize {
        self.src().len()
    }
    #[inline(always)]
    pub fn short_len(&self) -> U {
        self.src().len().try_into().unwrap()
    }
    fn from_span(src: &'src str, span: PathSpan<'src>) -> Self {
        Self(span.of(src))
    }
    pub fn from_prefix(src: &'src str) -> Result<Self, Error> {
        Ok(PathStr::from_span(src, PathSpan::new(src)?))
    }
    pub fn from_exact_match(src: &'src str) -> Result<Self, Error> {
        let span = PathSpan::new(src)?;
        if span.len() != src.len() {
            return Err(Error(ErrorKind::PathNoMatch, span.short_len()));
        }
        Ok(PathStr::from_span(src, span))
    }
    pub fn parts(&self) -> impl Iterator<Item = &'src str> {
        self.src().split('/')
    }
}
