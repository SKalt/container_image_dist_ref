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
    span::{impl_span_methods_on_tuple, IntoOption, OptionalSpan, Span, U},
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
            A::NoMatch => _Error(EKind::NoMatch, err.index()),
            A::TooLong => _Error(EKind::TooLong, err.index()),
            A::InvalidChar => _Error(EKind::InvalidChar, err.index()),
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
        // since 0-length OptionalHostOrPath will always have type Either, we can
        // safely downcast to the more specific PathSpan
        match ambiguous.kind() {
            Either | Path => Ok(if ambiguous.is_some() {
                Self(OptionalSpan::new(ambiguous.short_len()))
            } else {
                Self::none()
            }),
            Host => Err(Error(ErrorKind::PathInvalidChar, ambiguous.short_len())),
            IpV6 => Ok(Self(OptionalSpan::new(ambiguous.short_len()))),
        }
    }
}
impl IntoOption for PathSpan<'_> {
    fn is_some(&self) -> bool {
        self.short_len() > 0
    }
    fn none() -> Self {
        Self(OptionalSpan::new(0))
    }
}
impl<'src> PathSpan<'src> {
    fn none() -> Self {
        Self(OptionalSpan::new(0))
    }
    fn parse_component(src: &'src str) -> Result<Self, Error> {
        OptionalHostOrPath::new(src, PathKind::Path)
            .map_err(|e| Into::<_Error>::into(e))?
            .try_into()
    }
    /// parse an interior path-component starting from an '/' character at the given index
    /// in the source string
    // pub(crate) fn proceed_from(index: U, src: &'src str) -> Result<Self, Error> {
    //     let mut index = index;
    //     let rest = &src[index as usize..];
    //     match rest.bytes().next() {
    //         None => return Ok(Self::none()),
    //         Some(b'/') => index += 1,
    //         Some(_) => return Err(Error(ErrorKind::PathInvalidChar, index)),
    //     }

    //     // TODO: watch out for an infinite loop
    //     Self::parse_component(&rest[index as usize..])
    //         .map(|p| PathSpan(OptionalSpan::new(p.short_len() + index)))
    //         .map_err(|e| e + index)
    // }
    pub(crate) fn new(src: &'src str) -> Result<Self, Error> {
        let mut index = Self::parse_component(src)?.short_len();
        loop {
            let next = src[index as usize..].bytes().next();
            index = match next {
                None | Some(b':') => break,
                Some(b'/') => Ok(index + 1),
                Some(_) => Err(Error(ErrorKind::PathInvalidChar, index + 1)),
            }?;
            let rest = &src[index as usize..];
            let section = Self::parse_component(rest).map_err(|e| e + index)?;
            match section.into_option() {
                Some(p) => index += p.short_len(),
                None => break,
            }
        }
        Ok(Self(OptionalSpan::new(index)))
    }
}

pub struct PathStr<'src>(&'src str);
impl<'src> PathStr<'src> {
    pub(crate) fn src(&self) -> &'src str {
        self.0
    }
    fn from_span(src: &'src str, span: PathSpan<'src>) -> Self {
        Self(span.span_of(src))
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
impl SpanMethods<'_> for PathStr<'_> {
    fn short_len(&self) -> U {
        self.src().len().try_into().unwrap()
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_this() {
        use super::*;
        let src = "test.com/path:tag";
        let span = PathSpan::new(src).unwrap();
        assert_eq!(span.span_of(src), "test.com/path");
    }
}
