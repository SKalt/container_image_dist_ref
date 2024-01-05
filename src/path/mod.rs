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

use crate::domain::host::{scan_path_or_domain, Scan};
use crate::{Span, U};

pub enum Error {
    InvalidChar(U),
    TooLong(U), // implies didn't read after U::MAX
    NoMatch(U),
}
impl std::ops::Add<U> for Error {
    type Output = Self;
    fn add(self, rhs: U) -> Self {
        match self {
            Self::InvalidChar(len) => Self::InvalidChar(len + rhs),
            Self::TooLong(len) => Self::TooLong(len), // FIXME: avoid overflow
            Self::NoMatch(len) => Self::NoMatch(len + rhs),
        }
    }
}

fn separator(src: &str) -> Result<U, Error> {
    #[derive(Clone, Copy)]
    enum State {
        Dot,
        Underscore,
        DoubleUnderscore,
        Dashes(U),
        NoText,
    }
    let mut state = State::NoText;
    for c in src.bytes() {
        match (state, c) {
            (State::NoText, b'.') => state = State::Dot,
            (State::NoText, b'_') => state = State::Underscore,
            (State::NoText, b'-') => state = State::Dashes(1),
            (State::NoText, _) => return Err(Error::SeparatorInvalidChar(0)),
            (State::Underscore, b'_') => state = State::DoubleUnderscore,
            (State::Underscore, _) => break,
            (State::DoubleUnderscore, b'_' | b'.' | b'-') => {
                return Err(Error::SeparatorInvalidChar(3))
            }
            (State::DoubleUnderscore, _) => break,
            (State::Dashes(n), b'-') => state = State::Dashes(n + 1),
            (State::Dashes(U::MAX), _) => return Err(Error::TooLong(U::MAX)),
            (State::Dashes(_), _) => break,
            (State::Dot, _) => break,
        };
    }
    match state {
        State::Dot | State::Underscore => Ok(1),
        State::DoubleUnderscore => Ok(2),
        State::Dashes(n) => Ok(n),
        State::NoText => Err(Error::NoMatch(0)),
    }
}

fn alpha_numeric(src: &str) -> U {
    let mut len = 0;
    for c in src.bytes() {
        match c {
            b'a'..=b'z' | b'0'..=b'9' => {
                if len == U::MAX {
                    // guard against overflow
                    break;
                }
                len += 1
            }
            _ => break,
        }
    }
    len
}

pub(crate) struct PathSpan<'src>(Span<'src>);
impl<'src> PathSpan<'src> {
    #[inline(always)]
    fn len(&self) -> usize {
        self.0.as_len()
    }
    #[inline(always)]
    fn short_len(&self) -> U {
        self.0.short_len()
    }
    fn new(src: &'src str) -> Result<Self, Error> {
        let (len, scan) = scan_path_or_domain(src);
        if scan.valid_completed_path() {
            Ok(Self(Span::new(len)))
        } else if scan.last_was_invalid() || !scan.valid_component_end() {
            Err(Error::InvalidChar(len))
        } else {
            unreachable!()
        }
    }
}

fn component(src: &str) -> Result<U, Error> {
    let mut len = alpha_numeric(src).ok(0)?;
    loop {
        match separator(&src[len as usize..]).map_err(|e| e + len) {
            Ok(sep) => {
                len += sep;
                len += alpha_numeric(&src[len as usize..]).ok(len)?;
            }
            Err(e) => match e {
                Error::InvalidChar(_) => return Err(e),
                Error::TooLong(_) => return Err(e),
                Error::NoMatch(0) => break,
                _ => unreachable!(),
            },
        }
    }
    Ok(len)
}

fn path(src: &str) -> Result<U, Error> {
    let mut len = component(src)?;
    while let Some(b'/') = &src[len as usize..].bytes().next() {
        len += 1;
        len += component(&src[len as usize..]).map_err(|e| e + len)?;
    }
    Ok(len)
}

pub struct PathStr<'src> {
    pub src: &'src str,
}
impl<'src> PathStr<'src> {
    pub fn from_prefix(src: &'src str) -> Result<Self, Error> {
        let len = path(src)?;
        Ok(Self {
            src: &src[..len as usize],
        })
    }
    pub fn from_exact_match(src: &'src str) -> Result<Self, Error> {
        let len = path(src)?;
        if len == src.len().try_into().unwrap() {
            // FIXME: avoid unwrap
            Ok(Self { src })
        } else {
            Err(Error::NoMatch(len))
        }
    }
    pub fn parts(&self) -> impl Iterator<Item = &'src str> {
        self.src.split('/')
    }
}
