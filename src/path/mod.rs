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
        err::Kind::HostOrPathInvalidChar => err::Kind::PathInvalidChar,
        err::Kind::HostOrPathTooLong => err::Kind::PathTooLong,
        _ => e.kind(),
    };
    Error::at(e.index(), kind)
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) struct PathSpan<'src>(Length<'src>);
impl_span_methods_on_tuple!(PathSpan, Short);

impl IntoOption for PathSpan<'_> {
    fn is_some(&self) -> bool {
        self.short_len() > 0
    }
    fn none() -> Self {
        Self(Length::new(0))
    }
}
impl<'src> PathSpan<'src> {
    fn parse_component(src: &'src str) -> Result<Self, Error> {
        let parsed =
            Self::from_ambiguous(HostOrPathSpan::new(src, PathKind::Path).map_err(adapt_error)?)?;
        parsed
            .into_option()
            .ok_or(Error::at(0, err::Kind::PathComponentInvalidEnd))
    }
    pub(crate) fn parse_from_slash(src: &'src str) -> Result<Self, Error> {
        let mut index: Short = 0;
        loop {
            let next = src[index as usize..].bytes().next();
            index = match next {
                Some(b'/') => index
                    .checked_add(1)
                    .ok_or(Error::at(index, err::Kind::PathTooLong)),
                None | Some(b':') | Some(b'@') => break,
                Some(_) => Err(Error::at(index, err::Kind::PathInvalidChar)),
            }?;
            let rest = &src[index as usize..];
            let update = Self::parse_component(rest)
                .map_err(|e| {
                    e.index()
                        .checked_add(index)
                        .map(|i| Error::at(i, e.kind()))
                        .unwrap_or(Error::at(Short::MAX, err::Kind::PathTooLong))
                })?
                .into_option()
                .map(|p| p.short_len())
                .map(|len| {
                    index
                        .checked_add(len)
                        .ok_or(Error::at(Short::MAX, err::Kind::PathTooLong))
                });
            index = match update {
                None => break,
                Some(new_len) => new_len,
            }?;
        }
        Ok(Self(Length::new(index)))
    }
    pub(crate) fn extend(&self, rest: &'src str) -> Result<Self, Error> {
        let len = Self::parse_from_slash(rest)?
            .short_len()
            .checked_add(self.short_len())
            .ok_or(Error::at(Short::MAX, err::Kind::PathTooLong))?;
        Ok(Self(Length::new(len)))
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
    pub(crate) fn from_ambiguous(ambiguous: HostOrPathSpan<'src>) -> Result<Self, Error> {
        ambiguous
            .into_option()
            .map(|ambiguous| {
                ambiguous
                    .narrow(PathKind::Path)
                    .map(|disambiguated| Self(disambiguated.into_length()))
            })
            .unwrap_or(Ok(Self::none()))
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
