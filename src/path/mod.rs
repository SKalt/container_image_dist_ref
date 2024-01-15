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
    ambiguous::host_or_path::{HostOrPathSpan, Kind as PathKind},
    err,
    span::{impl_span_methods_on_tuple, IntoOption, Length, Lengthy, Short},
};
type Error = err::Error<Short>;
fn adapt_error(e: Error) -> Error {
    let kind = match e.kind() {
        err::Kind::HostOrPathNoMatch => err::Kind::PathNoMatch,
        // err::Kind::HostOrPathInvalidChar => err::Kind::PathComponentInvalidEnd,
        err::Kind::HostOrPathInvalidChar => err::Kind::PathInvalidChar,
        err::Kind::HostOrPathTooLong => err::Kind::PathTooLong,
        _ => e.kind(),
    };
    Error::at(e.index(), kind)
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) struct PathSpan<'src>(Length<'src>);
impl_span_methods_on_tuple!(PathSpan, Short);

impl<'src> TryFrom<HostOrPathSpan<'src>> for PathSpan<'src> {
    type Error = Error;
    fn try_from(ambiguous: HostOrPathSpan) -> Result<Self, Error> {
        use PathKind::*;
        // since 0-length HostOrPath will always have type Either, we can
        // safely downcast to the more specific PathSpan
        match ambiguous.kind() {
            Either | Path => Ok(if ambiguous.is_some() {
                Self(Length::new(ambiguous.short_len()))
            } else {
                Self::none()
            }),
            Host => Err(Error::at(ambiguous.short_len(), err::Kind::PathInvalidChar)),
            IpV6 => Ok(Self(Length::new(ambiguous.short_len()))),
        }
    }
}
impl IntoOption for PathSpan<'_> {
    fn is_some(&self) -> bool {
        self.short_len() > 0
    }
    fn none() -> Self {
        Self(Length::new(0))
    }
}
impl<'src> PathSpan<'src> {
    fn none() -> Self {
        Self(Length::new(0))
    }
    fn parse_component(src: &'src str) -> Result<Self, Error> {
        HostOrPathSpan::new(src, PathKind::Path)
            .map_err(adapt_error)?
            .try_into()
    }
    pub(crate) fn parse_from_slash(src: &'src str) -> Result<Self, Error> {
        let mut index: Short = 0;
        loop {
            let next = src[index as usize..].bytes().next();
            match next {
                None | Some(b':') | Some(b'@') => break,
                Some(b'/') => Ok(()),
                Some(_) => Err(Error::at(index, err::Kind::PathInvalidChar)),
            }?;
            index = index
                .checked_add(1)
                .ok_or(Error::at(index, err::Kind::PathTooLong))?;
            let rest = &src[index as usize..];
            let section = Self::parse_component(rest).map_err(|e| e + index)?;
            index = match section.into_option() {
                Some(p) => index
                    .checked_add(p.short_len())
                    .ok_or(Error::at(Short::MAX, err::Kind::PathTooLong))?,
                None => break,
            }
        }
        Ok(Self(Length::new(index)))
    }
    pub(crate) fn new(src: &'src str) -> Result<Self, Error> {
        let index = Self::parse_component(src)?.short_len();
        let result = Self::parse_from_slash(&src[index as usize..]).map_err(|e| {
            index
                .checked_add(e.index())
                .map(|i| Error::at(i, e.kind()))
                .unwrap_or(Error::at(Short::MAX, err::Kind::PathTooLong))
        })?;
        let len = result
            .short_len()
            .checked_add(index)
            .ok_or(Error::at(Short::MAX, err::Kind::PathTooLong))?;
        Ok(Self(Length::new(len)))
    }
    pub(crate) fn from_ambiguous(
        ambiguous: HostOrPathSpan<'src>,
        context: &'src str,
    ) -> Result<Self, Error> {
        match ambiguous.kind() {
            PathKind::Either | PathKind::Path => Ok(if ambiguous.is_some() {
                Self(ambiguous.into_length())
            } else {
                Self::none()
            }),
            PathKind::Host => Error::at(
                ambiguous.span_of(context)
                    .bytes().enumerate()
                    .find(|(_, b)| b.is_ascii_uppercase())
                    .map(|(i, _)| i)
                    .unwrap() // safe since ambiguous.kind == Host, which means there must be an uppercase letter
                    .try_into()
                    .unwrap(), // safe since ambiguous.span_of(context) must be short
                err::Kind::PathInvalidChar,
            )
            .into(),
            PathKind::IpV6 => Ok(Self(ambiguous.into_length())),
        }
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
            return Error::at(span.short_len(), err::Kind::PathNoMatch).into();
        }
        Ok(PathStr::from_span(src, span))
    }
    pub fn parts(&self) -> impl Iterator<Item = &'src str> {
        self.src().split('/')
    }
}
impl Lengthy<'_, Short> for PathStr<'_> {
    fn short_len(&self) -> Short {
        self.src().len().try_into().unwrap()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_this() {
        let src = "test.com/path:tag";
        let span = PathSpan::new(src).unwrap();
        assert_eq!(span.span_of(src), "test.com/path");
    }
}
