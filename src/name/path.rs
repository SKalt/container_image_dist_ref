//! # Path parsing
//!
//! The current RC of the OCI distribution spec and distribution/reference agree
//! on the format of a path component. The OCI distribution spec's current RC is
//! a strict superset of the previous path pattern, so implementing the newer, more-
//! compatible pattern shouldn't result in any previously-valid paths being rejected.
//!

// {{{sh
//    cat ../grammars/oci_name.ebnf | sed 's#^#//! #g';
//    printf '//! ```\n\n// '
// }}}{{{out skip=2

//! ```ebnf
//! path                ::= path-component ("/" path-component)*
//! path-component      ::= [a-z0-9]+ (separator [a-z0-9]+)*
//! separator           ::= [_.] | "__" | "-"+
//! ```

// }}}

// for more context, see:
// -- https://github.com/opencontainers/distribution-spec/blob/v1.0.1/spec.md#pulling-manifests
// -- https://github.com/opencontainers/distribution-spec/blob/v1.1.0-rc3/spec.md#pulling-manifests
// -- https://github.com/opencontainers/distribution-spec/commit/a73835700327bd1c037e33d0834c46ff98ac1286
// -- https://github.com/opencontainers/distribution-spec/commit/efe2de09470d7f182d2fbd83ac4462fbdc462455

use core::num::NonZeroU8;

use crate::{
    ambiguous::host_or_path::{HostOrPathSpan, Kind as PathKind},
    err,
    span::{impl_span_methods_on_tuple, Length, Lengthy, ShortLength},
};
type Error = err::Error<u8>;

/// adapt ambiguous error kinds into path-specific error kinds
fn map_error(e: Error) -> Error {
    let kind = match e.kind() {
        err::Kind::HostOrPathMissing => err::Kind::PathMissing,
        err::Kind::HostOrPathInvalidChar => err::Kind::PathInvalidChar,
        err::Kind::HostOrPathTooLong => err::Kind::PathTooLong,
        _ => e.kind(),
    };
    Error::at(e.index(), kind)
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) struct PathSpan<'src>(ShortLength<'src>);
impl_span_methods_on_tuple!(PathSpan, u8, NonZeroU8);

const ERR_PATH_TOO_LONG: Error = Error::at(u8::MAX, err::Kind::PathTooLong);

impl<'src> PathSpan<'src> {
    fn parse_component(src: &'src str) -> Result<Self, Error> {
        let ambiguous = HostOrPathSpan::new(src, PathKind::Path).map_err(map_error)?;
        Self::try_from(ambiguous)
    }
    pub(crate) fn parse_from_slash(src: &'src str) -> Result<Option<Self>, Error> {
        let mut index: u8 = 0;
        loop {
            let next = src[index as usize..].bytes().next();
            index = match next {
                Some(b'/') => index.checked_add(1).ok_or(err::Kind::PathTooLong),
                None | Some(b':') | Some(b'@') => break,
                Some(_) => Err(err::Kind::PathInvalidChar),
            }
            .map_err(|kind| Error::at(index, kind))?;
            let rest = &src[index as usize..];
            let component = Self::parse_component(rest).map_err(|e| {
                let kind = match e.kind() {
                    err::Kind::PathMissing => err::Kind::PathComponentInvalidEnd,
                    kind => kind,
                };
                e.index()
                    .checked_add(index)
                    .map(|i| Error::at(i, kind))
                    .unwrap_or(ERR_PATH_TOO_LONG)
            })?;
            index = index
                .checked_add(component.short_len().into())
                .ok_or(ERR_PATH_TOO_LONG)?;
        }
        Ok(Length::new(index).map(Self))
    }
    pub(crate) fn extend(self, rest: &'src str) -> Result<Self, Error> {
        let extension = Self::parse_from_slash(rest).map_err(|e| {
            e.index()
                .checked_add(self.short_len().into())
                .map(|i| Error::at(i, e.kind()))
                .unwrap_or(ERR_PATH_TOO_LONG)
        })?;
        let len = self
            .short_len()
            .checked_add(extension.map(|e| e.short_len().into()).unwrap_or(0))
            .ok_or(ERR_PATH_TOO_LONG)?;
        Ok(Self(Length::from_nonzero(len)))
    }
    pub fn new(src: &'src str) -> Result<Self, Error> {
        let first_component = Self::parse_component(src)?;
        first_component.extend(&src[first_component.len()..])
    }
}

impl<'src> TryFrom<HostOrPathSpan<'src>> for PathSpan<'src> {
    type Error = Error;
    fn try_from(ambiguous: HostOrPathSpan<'src>) -> Result<Self, Error> {
        ambiguous
            .narrow(PathKind::Path)
            .map(|disambiguated| Self(Length::from_nonzero(disambiguated.short_len())))
    }
}

/// Not including any leading `/`.
pub struct Path<'src> {
    src: &'src str,
    span: PathSpan<'src>,
}
impl<'src> Path<'src> {
    #[inline]
    pub(crate) fn from_span(span: PathSpan<'src>, src: &'src str) -> Self {
        debug_assert_eq!(span.len(), src.len(), "src: {src:?}");
        // TODO: enforce exact-match invariant on all from_span methods
        Self { src, span }
    }
    /// Parse a path string NOT starting with a leading `/`. Parsing continues until it
    /// reaches a `:`, `@`, or the end of the string.
    pub fn new(src: &'src str) -> Result<Self, Error> {
        let span = PathSpan::new(src)?;
        Ok(Self::from_span(span, src))
    }
    #[allow(missing_docs)]
    pub fn to_str(&self) -> &'src str {
        self.span.span_of(self.src)
    }
    /// Yields an iterator over the `/`-delimited components of the path.
    pub fn parts(&self) -> impl Iterator<Item = &'src str> {
        self.to_str().split('/')
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_this() {
        // some strings in front of "/" must be paths since they include a underscores:
        let src = "not_a_host/path:tag";
        let span = PathSpan::new(src).unwrap();
        assert_eq!(span.span_of(src), "not_a_host/path");

        // watch out, though: host names are also valid paths
        let src = "test.com/path:tag";
        let span = PathSpan::new(src).unwrap();
        assert_eq!(span.span_of(src), "test.com/path");
    }
}
