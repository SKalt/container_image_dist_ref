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
    ambiguous::host_or_path::{Kind as PathKind, OptionalHostOrPath},
    err::{self, Error},
    span::{impl_span_methods_on_tuple, IntoOption, Length, Lengthy, Short},
};

fn adapt_error(e: Error) -> Error {
    let kind = match e.kind() {
        err::Kind::HostOrPathNoMatch => err::Kind::PathNoMatch,
        // err::Kind::HostOrPathInvalidChar => err::Kind::PathComponentInvalidEnd,
        err::Kind::HostOrPathInvalidChar => err::Kind::PathInvalidChar,
        err::Kind::HostOrPathTooLong => err::Kind::PathTooLong,
        _ => e.kind(),
    };
    Error(e.index(), kind)
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) struct OptionalPathSpan<'src>(Length<'src>);
impl_span_methods_on_tuple!(OptionalPathSpan, Short);

impl<'src> TryFrom<OptionalHostOrPath<'src>> for OptionalPathSpan<'src> {
    type Error = Error;
    fn try_from(ambiguous: OptionalHostOrPath) -> Result<Self, Error> {
        use PathKind::*;
        // since 0-length OptionalHostOrPath will always have type Either, we can
        // safely downcast to the more specific PathSpan
        match ambiguous.kind() {
            Either | Path => Ok(if ambiguous.is_some() {
                Self(Length::new(ambiguous.short_len()))
            } else {
                Self::none()
            }),
            Host => Err(Error(ambiguous.short_len(), err::Kind::PathInvalidChar)),
            IpV6 => Ok(Self(Length::new(ambiguous.short_len()))),
        }
    }
}
impl IntoOption for OptionalPathSpan<'_> {
    fn is_some(&self) -> bool {
        self.short_len() > 0
    }
    fn none() -> Self {
        Self(Length::new(0))
    }
}
impl<'src> OptionalPathSpan<'src> {
    fn none() -> Self {
        Self(Length::new(0))
    }
    fn parse_component(src: &'src str) -> Result<Self, Error> {
        OptionalHostOrPath::new(src, PathKind::Path)
            .map_err(adapt_error)?
            .try_into()
    }
    pub(crate) fn parse_from_slash(src: &'src str) -> Result<Self, Error> {
        let mut index: Short = 0;
        loop {
            let next = src[index as usize..].bytes().next();
            index = match next {
                None | Some(b':') | Some(b'@') => break,
                Some(b'/') => Ok(index + 1),
                Some(_) => Err(Error(index + 1, err::Kind::PathInvalidChar)),
            }?;
            let rest = &src[index as usize..];
            let section = Self::parse_component(rest).map_err(|e| e + index)?;
            match section.into_option() {
                Some(p) => index += p.short_len(),
                None => break,
            }
        }
        Ok(Self(Length::new(index)))
    }
    pub(crate) fn new(src: &'src str) -> Result<Self, Error> {
        let index = Self::parse_component(src)?.short_len();
        Self::parse_from_slash(&src[index as usize..])
            .map(|p| Self(Length::new(p.short_len() + index)))
            .map_err(|e| e + index)
    }
    pub(crate) fn from_ambiguous(
        ambiguous: OptionalHostOrPath<'src>,
        context: &'src str,
    ) -> Result<Self, Error> {
        match ambiguous.kind() {
            PathKind::Either | PathKind::Path => Ok(if ambiguous.is_some() {
                Self(ambiguous.into_span())
            } else {
                Self::none()
            }),
            PathKind::Host => Err(Error(
                ambiguous.span_of(context)
                    .bytes()
                    .find(|b| b.is_ascii_uppercase())
                    .unwrap() // safe since ambiguous.kind == Host, which means there must be an uppercase letter
                    .try_into()
                    .unwrap(), // safe since ambiguous.span_of(context) must be short
                err::Kind::PathInvalidChar,
            )),
            PathKind::IpV6 => Ok(Self(ambiguous.into_span())),
        }
    }
}

pub struct PathStr<'src>(&'src str);
impl<'src> PathStr<'src> {
    pub(crate) fn src(&self) -> &'src str {
        self.0
    }
    fn from_span(src: &'src str, span: OptionalPathSpan<'src>) -> Self {
        Self(span.span_of(src))
    }
    pub fn from_prefix(src: &'src str) -> Result<Self, Error> {
        Ok(PathStr::from_span(src, OptionalPathSpan::new(src)?))
    }
    pub fn from_exact_match(src: &'src str) -> Result<Self, Error> {
        let span = OptionalPathSpan::new(src)?;
        if span.len() != src.len() {
            return Err(Error(span.short_len(), err::Kind::PathNoMatch));
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
        let span = OptionalPathSpan::new(src).unwrap();
        assert_eq!(span.span_of(src), "test.com/path");
    }
}
