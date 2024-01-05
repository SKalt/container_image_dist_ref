// > Grammar
// >
// > ```ebnf
// > reference                       := name [ ":" tag ] [ "@" digest ]
// > name                            := [domain '/'] remote-name
// > domain                          := host [':' port-number]
// > host                            := domain-name | IPv4address | \[ IPv6address \] ; rfc3986 appendix-A
// > domain-name                     := domain-component ['.' domain-component]*
// > domain-component                := /([a-zA-Z0-9]|[a-zA-Z0-9][a-zA-Z0-9-]*[a-zA-Z0-9])/
// > port-number                     := /[0-9]+/
// > path-component                  := alpha-numeric [separator alpha-numeric]*
// > path (or "remote-name")         := path-component ['/' path-component]*
// > alpha-numeric                   := /[a-z0-9]+/
// > separator                       := /[_.]|__|[-]*/
// >
// > tag                             := /[\w][\w.-]{0,127}/
// >
// > digest                          := digest-algorithm ":" digest-hex
// > digest-algorithm                := digest-algorithm-component [ digest-algorithm-separator digest-algorithm-component ]*
// > digest-algorithm-separator      := /[+.-_]/
// > digest-algorithm-component      := /[A-Za-z][A-Za-z0-9]*/
// > digest-hex                      := /[0-9a-fA-F]{32,}/ ; At least 128 bit digest value
// >
// > identifier                      := /[a-f0-9]{64}/
// > ```
// >
// > -- https://github.com/distribution/reference/blob/v0.5.0/reference.go#L4-L26
// > -- https://github.com/distribution/reference/blob/4894124079e525c3c3c5c8aacaa653b5499004e9/reference.go#L4-L26

// https://docs.rs/regex/latest/regex/#sharing-a-regex-across-threads-can-result-in-contention

pub mod digest;
pub mod domain;
pub mod name;
pub mod path;
pub mod tag;

use std::marker::PhantomData;

use digest::Compliance;
use tag::TagSpan;

use self::{
    digest::{Digest, Error as DigestError},
    name::{Error as NameError, Name},
    path::Error as PathError,
    tag::{Error as TagError, TagStr},
};

pub type U = u8; // HACK: arbitrary limit

/// to avoid lugging around an entire &str, we can use a span to represent a
/// length of string with a lifetime tied to the original string slice.

#[derive(Clone, Copy)]
struct Span<'src> {
    __phantom: PhantomData<&'src str>, // tie Span to the lifetime of a string slice
    len: U,
}
impl<'src> Span<'src> {
    fn new(len: U) -> Self {
        Self {
            __phantom: PhantomData,
            len,
        }
    }
    fn span_of(&self, src: &'src str) -> &'src str {
        &src[..self.len as usize]
    }
    fn short_len(&self) -> U {
        self.len
    }
    fn as_len(&self) -> usize {
        self.len as usize
    }
}

pub enum Error {
    TooLong,
    Name(NameError),
    Tag(TagError),
    Digest(DigestError),
    NoMatch(u8),
}
impl From<NameError> for Error {
    fn from(err: NameError) -> Self {
        Self::Name(err)
    }
}
impl From<DigestError> for Error {
    fn from(err: DigestError) -> Self {
        Self::Digest(err)
    }
}
impl From<TagError> for Error {
    fn from(err: TagError) -> Self {
        Self::Tag(err)
    }
}

pub struct Reference<'src> {
    src: &'src str,
    pub name: Name<'src>,
    pub tag: Option<TagSpan<'src>>,
    pub digest: Option<Digest<'src>>,
}
impl<'src> Reference<'src> {
    pub fn new(src: &'src str) -> Result<(Self, Compliance), Error> {
        if src.len() == 0 {
            return Err(Error::NoMatch(0));
        }
        U::try_from(src.len()).map_err(|_| Error::TooLong)?; // check length addressable by integer size
        let name = Name::from_prefix(src)?;
        let mut len = name.len();

        let tag = if src[name.len()..].bytes().next() == Some(b':') {
            // consume the separator colon
            len += 1;
            let tag = TagSpan::new(&src[len..])?;
            len += tag.len();

            Some(tag)
        } else {
            None
        };
        let mut compliance = Compliance::Universal;
        let digest = if let Some(next) = src[len..].bytes().next() {
            len += 1;
            if next == b'@' {
                let (digest, _compliance) = Digest::new(&src[len..])?;
                compliance = _compliance;
                Some(digest)
            } else if let Some(_) = tag {
                return Err(Error::Tag(TagError::InvalidChar(len.try_into().unwrap())));
            } else {
                return Err(Error::Name(NameError::Path(PathError::NoMatch(
                    len.try_into().unwrap(),
                ))));
            }
        } else {
            None
        };
        Ok((
            Reference {
                src,
                name,
                tag,
                digest,
            },
            compliance,
        ))
    }
}
