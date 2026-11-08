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
const fn map_error(e: Error) -> Error {
    let kind = match e.kind() {
        err::Kind::HostOrPathMissing => err::Kind::PathMissing,
        err::Kind::HostOrPathInvalidChar => err::Kind::PathInvalidChar,
        err::Kind::HostOrPathTooLong => err::Kind::PathTooLong,
        _ => e.kind(),
    };
    Error::at(e.index(), kind)
}
/// max length = `u8::MAX` = 255
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
        let bytes = src.as_bytes();
        match bytes.first().copied() {
            None => return Ok(None),
            // a path can be empty when the tag or digest marker comes first
            Some(b':' | b'@') => return Ok(None),
            Some(b'/') => {}
            Some(_) => return Error::at(0, err::Kind::PathInvalidChar).into(),
        }
        // `index` is capped at u8::MAX below, so these conversions cannot truncate.
        let at = |i: usize, kind: err::Kind| Error::at(u8::try_from(i).unwrap_or(u8::MAX), kind);

        // Scan all `/`-separated components in a single pass instead of parsing
        // each component separately. The state below mirrors the component
        // grammar: `[a-z0-9]+ (separator [a-z0-9]+)*`.
        let mut index: usize = 0;
        // immediately after a '/', a component must start with [a-z0-9]
        let mut component_start = false;
        let mut underscore_count: u8 = 0;
        let mut last_was_dot = false;
        let mut last_was_dash = false;

        while let Some(&c) = bytes.get(index) {
            if matches!(c, b':' | b'@') {
                break;
            }
            // The path (including the leading '/') is limited to u8::MAX bytes.
            if index >= usize::from(u8::MAX) {
                return Error::at(u8::MAX, err::Kind::PathTooLong).into();
            }
            match c {
                b'/' => {
                    if component_start {
                        // empty component, e.g. "//" or "/path//"
                        return at(index, err::Kind::PathComponentInvalidEnd).into();
                    }
                    if last_was_dot || last_was_dash || underscore_count > 0 {
                        return at(
                            index.saturating_sub(1),
                            err::Kind::HostOrPathInvalidComponentEnd,
                        )
                        .into();
                    }
                    component_start = true;
                    underscore_count = 0;
                    last_was_dot = false;
                    last_was_dash = false;
                }
                b'a'..=b'z' | b'0'..=b'9' => {
                    component_start = false;
                    underscore_count = 0;
                    last_was_dot = false;
                    last_was_dash = false;
                }
                _ if component_start => {
                    // components cannot start with a separator, an uppercase letter,
                    // or any other invalid character
                    return at(index, err::Kind::PathInvalidChar).into();
                }
                b'_' => {
                    if last_was_dot || last_was_dash {
                        return at(index, err::Kind::HostOrPathInvalidComponentEnd).into();
                    }
                    let next = underscore_count.saturating_add(1);
                    if next > 2 {
                        return at(index, err::Kind::PathInvalidChar).into();
                    }
                    underscore_count = next;
                }
                b'.' => {
                    if last_was_dot || last_was_dash || underscore_count > 0 {
                        return at(index, err::Kind::HostOrPathInvalidComponentEnd).into();
                    }
                    last_was_dot = true;
                }
                b'-' => {
                    if last_was_dot || underscore_count > 0 {
                        return at(index, err::Kind::HostOrPathInvalidComponentEnd).into();
                    }
                    last_was_dash = true;
                }
                _ => return at(index, err::Kind::PathInvalidChar).into(),
            }
            index = index.saturating_add(1);
        }

        if component_start {
            // a trailing '/' leaves an empty final component
            return at(index, err::Kind::PathComponentInvalidEnd).into();
        }
        if last_was_dot || last_was_dash || underscore_count > 0 {
            return at(
                index.saturating_sub(1),
                err::Kind::HostOrPathInvalidComponentEnd,
            )
            .into();
        }
        match u8::try_from(index) {
            Ok(len) => Ok(Length::new(len).map(Self)),
            Err(_) => Error::at(u8::MAX, err::Kind::PathTooLong).into(),
        }
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
        Ok(Self::from_span(span, &src[..span.len()]))
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
#[allow(clippy::unwrap_used)]
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

    #[test]
    fn new_stops_at_tag_or_digest() {
        assert_eq!(Path::new("repo:tag").unwrap().to_str(), "repo");
        assert_eq!(Path::new("repo@algo:encoded").unwrap().to_str(), "repo");
        assert_eq!(Path::new("repo").unwrap().to_str(), "repo");
    }

    #[test]
    fn single_pass_matches_component_parser() {
        // Reference implementation of the previous, component-at-a-time parser.
        fn reference(src: &str) -> Result<Option<u8>, (err::Kind, u8)> {
            let mut index = 0u8;
            loop {
                index = match src.as_bytes().get(index as usize) {
                    Some(b'/') => index
                        .checked_add(1)
                        .ok_or((err::Kind::PathTooLong, u8::MAX))?,
                    None | Some(b':' | b'@') => break,
                    Some(_) => return Err((err::Kind::PathInvalidChar, index)),
                };
                let component = HostOrPathSpan::new(&src[index as usize..], PathKind::Path)
                    .map_err(map_error)
                    .map_err(|e| {
                        let kind = match e.kind() {
                            err::Kind::PathMissing => err::Kind::PathComponentInvalidEnd,
                            kind => kind,
                        };
                        (kind, e.index().saturating_add(index))
                    })?;
                index = index
                    .checked_add(u8::from(component.short_len()))
                    .ok_or((err::Kind::PathTooLong, u8::MAX))?;
            }
            Ok(match index {
                0 => None,
                len => Some(len),
            })
        }

        fn actual(src: &str) -> Result<Option<u8>, (err::Kind, u8)> {
            PathSpan::parse_from_slash(src)
                .map(|parsed| {
                    parsed.map(|span| {
                        #[allow(clippy::cast_possible_truncation)]
                        let len = span.len() as u8;
                        len
                    })
                })
                .map_err(|e| (e.kind(), e.index()))
        }

        // Exhaustively compare short strings over every class of relevant character.
        let alphabet = b"/a0_.-:@U$";
        let mut buf = [0u8; 4];
        for len in 0..=buf.len() {
            let power = u32::try_from(len).unwrap();
            let combinations = alphabet.len().pow(power);
            for mut n in 0..combinations {
                for slot in buf.iter_mut().take(len) {
                    *slot = alphabet[n % alphabet.len()];
                    n /= alphabet.len();
                }
                let src = core::str::from_utf8(&buf[..len]).unwrap();
                assert_eq!(actual(src), reference(src), "path: {src:?}");
            }
        }
    }
}
