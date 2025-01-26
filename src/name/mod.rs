/*!
# Image names: optional domain and a required path

```ebnf
name ::= (domain "/")? path
```
*/
use core::num::NonZeroU16;

use crate::span::{nonzero, Lengthy, OptionallyZero};

use self::domain::{Domain, DomainSpan};

pub mod domain;
pub mod path;

/// the maximum total number of characters in a repository name, as defined by
/// <https://github.com/distribution/reference/blob/main/reference.go#L39>
pub const MAX_LEN: u8 = 255;

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) struct NameSpan<'src> {
    pub(crate) domain: Option<DomainSpan<'src>>,
    // All valid refs have a non-empty path
    pub(crate) path: path::PathSpan<'src>,
}
impl Lengthy<'_, u16, NonZeroU16> for NameSpan<'_> {
    #[inline]
    fn short_len(&self) -> NonZeroU16 {
        let len = self.path.short_len().widen().upcast().saturating_add(
            self.domain
                .map(|d| d.short_len().upcast().saturating_add(1)) // +1 for the leading '/'
                .unwrap_or(0),
        );
        nonzero!(u16, len)
    }
}

/// Includes the domain and path portions of an image reference.
pub struct Name<'src> {
    src: &'src str,
    span: NameSpan<'src>,
}

impl<'src> Name<'src> {
    // the logic for constructing a name is tricky due to the domain:port/name:tag
    // ambiguity, so adding a `fn new(&str) -> Self` constructor is a TODO for later

    #[inline]
    pub(crate) fn from_span(span: NameSpan<'src>, src: &'src str) -> Self {
        debug_assert_eq!(span.len(), src.len());
        Self { src, span }
    }
    /// Returns the domain part of the name, if it exists.
    pub fn domain(&self) -> Option<Domain<'_>> {
        self.span
            .domain
            .map(|span| Domain::from_span(span, span.span_of(self.src)))
    }
    /// Returns the path part of the name, which always exists.
    pub fn path(&self) -> path::Path<'_> {
        let start = self
            .span
            .domain
            .map(|d| d.len().saturating_add(1))
            .unwrap_or(0);
        let src = &self.src[start..];
        path::Path::from_span(self.span.path, src)
    }
    #[allow(missing_docs)]
    pub fn to_str(&self) -> &str {
        self.span.span_of(self.src)
    }
}
