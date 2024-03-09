use self::domain::DomainSpan;

pub mod domain;
pub mod path;

/// the maximum total number of characters in a repository name, as defined by
/// <https://github.com/distribution/reference/blob/main/reference.go#L39>
pub const MAX_LEN: u8 = 255;

#[derive(Clone, Copy, PartialEq, Eq)]
pub struct NameSpan<'src> {
    pub(crate) domain: Option<DomainSpan<'src>>,
    // All valid refs have a non-empty path
    pub(crate) path: path::PathSpan<'src>,
}
