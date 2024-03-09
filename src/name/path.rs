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
    span::{impl_span_methods_on_tuple, Length, Lengthy, OptionallyZero, ShortLength},
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

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) struct PathSpan<'src>(ShortLength<'src>);
impl_span_methods_on_tuple!(PathSpan, u8, NonZeroU8);

const ERR_PATH_TOO_LONG: Error = Error::at(u8::MAX, err::Kind::PathTooLong);

impl<'src> PathSpan<'src> {
    fn parse_component(src: &'src str) -> Result<Option<Self>, Error> {
        let ambiguous = HostOrPathSpan::new(src, PathKind::Path).map_err(map_error)?;
        let result = if let Some(ambiguous) = ambiguous {
            Some(Self::from_ambiguous(ambiguous)?)
        } else {
            None
        };
        Ok(result)
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
            let update = Self::parse_component(rest)
                .map_err(|e| {
                    e.index()
                        .checked_add(index)
                        .map(|i| Error::at(i, e.kind()))
                        .unwrap_or(ERR_PATH_TOO_LONG)
                })?
                .map(|p| p.short_len().into())
                .map(|len| index.checked_add(len).ok_or(ERR_PATH_TOO_LONG));
            if let Some(update) = update {
                index = update?;
            } else {
                return Error::at(index, err::Kind::PathComponentInvalidEnd).into();
            }
        }
        Ok(Length::new(index).map(Self))
    }
    pub(crate) fn extend(&self, rest: &'src str) -> Result<Self, Error> {
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
    pub fn new(src: &'src str) -> Result<Option<Self>, Error> {
        let first_component = Self::parse_component(src)?;
        if let Some(first_component) = first_component {
            first_component
                .extend(&src[first_component.short_len().as_usize()..])
                .map(Some)
        } else {
            Ok(None)
        }
    }
    pub(crate) fn from_ambiguous(ambiguous: HostOrPathSpan<'src>) -> Result<Self, Error> {
        ambiguous
            .narrow(PathKind::Path)
            .map(|disambiguated| Self(Length::from_nonzero(disambiguated.short_len())))
    }
}

pub struct PathStr<'src> {
    src: &'src str,
    span: PathSpan<'src>,
}
impl<'src> PathStr<'src> {
    pub fn new(src: &'src str) -> Result<Option<Self>, Error> {
        Ok(PathSpan::new(src)?.map(|span| Self { src, span }))
    }
    pub fn src(&self) -> &'src str {
        self.span.span_of(self.src)
    }
    pub fn parts(&self) -> impl Iterator<Item = &'src str> {
        self.src().split('/')
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_this() {
        // some strings in front of "/" must be paths since they include a underscores:
        let src = "not_a_host/path:tag";
        let span = PathSpan::new(src).unwrap().unwrap();
        assert_eq!(span.span_of(src), "not_a_host/path");

        // watch out, though: host names are also valid paths
        let src = "test.com/path:tag";
        let span = PathSpan::new(src).unwrap().unwrap();
        assert_eq!(span.span_of(src), "test.com/path");
    }
}
