// > name                            := [domain '/'] remote-name
// > domain                          := host [':' port-number]
// > port-number                     := /[0-9]+/
// > host                            := domain-name | IPv4address | \[ IPv6address \] ; rfc3986 appendix-A
// > domain-name                     := domain-component ['.' domain-component]*
// > domain-component                := alpha-numeric [ ( alpha-numeric | '-' )* alpha-numeric ]
// > path-component                  := alpha-numeric [separator alpha-numeric]*
// > path (or "remote-name")         := path-component ['/' path-component]*
// > alpha-numeric                   := /[a-z0-9]+/
// > separator                       := /[_.]|__|[-]*/
//
// Note that domain components conflict with path components:
// | class | domain-component | path-component |
// | ----- | ---------------- | -------------- |
// | upper | yes              | no             |
// | -     | inner            | inner          |
// | _     | no               | inner          |
// | .     | yes              | yes            |

use crate::{
    ambiguous::{
        host_or_path::{Error as HostOrPathError, Kind as HostOrPathKind, OptionalHostOrPath},
        port_or_tag::{Error as PortOrTagError, Kind as PortOrTagKind, OptionalPortOrTag},
    },
    span::{IntoOption, SpanMethods, U},
};
#[cfg_attr(test, derive(Debug))]
#[derive(Clone, Copy)]
pub(crate) enum ErrKind {
    // could apply to either host or path --------------------------------------
    LeftNoMatch,
    LeftInvalidChar,
    LeftTooLong,
    // could apply to either tag or port --------------------------------------
    RightInvalidChar,
    RightTooLong,
    // right is always optional, so no NoMatch variant
    // ipv6-specific errors ----------------------------------------------------
    Ipv6NoMatch,
    Ipv6TooLong,
    Ipv6BadColon,
    Ipv6TooManyHexDigits,
    Ipv6TooManyGroups,
    Ipv6TooFewGroups,
    Ipv6MissingClosingBracket,
}
#[cfg_attr(test, derive(Debug))]
pub(crate) struct Error(ErrKind, U);
impl Error {
    pub(crate) fn kind(&self) -> ErrKind {
        self.0
    }
    pub(crate) fn index(&self) -> U {
        self.1
    }
}

impl From<HostOrPathError> for Error {
    fn from(err: HostOrPathError) -> Self {
        use crate::ambiguous::host_or_path::AmbiguousErrorKind as A;
        match err.kind() {
            A::NoMatch => Error(ErrKind::LeftNoMatch, err.index()),
            A::TooLong => Error(ErrKind::LeftTooLong, err.index()),
            A::InvalidChar => Error(ErrKind::LeftInvalidChar, err.index()),
            A::Ipv6NoMatch => Error(ErrKind::Ipv6NoMatch, err.index()),
            A::Ipv6TooLong => Error(ErrKind::Ipv6TooLong, err.index()),
            A::Ipv6BadColon => Error(ErrKind::Ipv6BadColon, err.index()),
            A::Ipv6TooManyHexDigits => Error(ErrKind::Ipv6TooManyHexDigits, err.index()),
            A::Ipv6TooManyGroups => Error(ErrKind::Ipv6TooManyGroups, err.index()),
            A::Ipv6TooFewGroups => Error(ErrKind::Ipv6TooFewGroups, err.index()),
            A::Ipv6MissingClosingBracket => Error(ErrKind::Ipv6MissingClosingBracket, err.index()),
        }
    }
}
impl From<PortOrTagError> for Error {
    fn from(value: PortOrTagError) -> Self {
        match value {
            PortOrTagError::InvalidChar(len) => Error(ErrKind::RightInvalidChar, len),
            PortOrTagError::TooLong(len) => Error(ErrKind::RightTooLong, len),
        }
    }
}
impl std::ops::Add<U> for Error {
    type Output = Self;
    fn add(self, rhs: U) -> Self {
        Self(self.0, self.1 + rhs)
    }
}

pub(crate) struct DomainOrRefSpan<'src> {
    pub(crate) host_or_path: OptionalHostOrPath<'src>,
    pub(crate) optional_port_or_tag: OptionalPortOrTag<'src>,
}
impl SpanMethods<'_> for DomainOrRefSpan<'_> {
    fn short_len(&self) -> U {
        self.host_or_path.short_len() + self.optional_port_or_tag.short_len()
    }
}
impl<'src> DomainOrRefSpan<'src> {
    pub(crate) fn new(src: &'src str) -> Result<Self, Error> {
        let mut host_or_path = OptionalHostOrPath::new(src, HostOrPathKind::Either)?;
        let rest = &src[host_or_path.len()..];
        let next_ascii_char = rest.bytes().next();
        let port_or_tag = match next_ascii_char {
            Some(b':') => OptionalPortOrTag::new(rest, PortOrTagKind::Either)
                .map_err(|e| e.into())
                .map_err(|e: Error| e + host_or_path.short_len()),
            Some(b'/') => {
                host_or_path =
                    host_or_path
                        .narrow(HostOrPathKind::Host)
                        .map_err(|e| match e.kind() {
                            super::host_or_path::AmbiguousErrorKind::InvalidChar => {
                                let offending_char = src
                                    .find(|c| char::is_ascii_uppercase(&c))
                                    .unwrap() // safe since this .narrow(Host) only throws this error if there was an uppercase letter
                                    .try_into()
                                    .unwrap(); // safe since host_or_path.len() is a valid short index into src
                                Error(ErrKind::LeftInvalidChar, offending_char)
                            }
                            _ => e.into(),
                        })?;
                OptionalPortOrTag::new(rest, PortOrTagKind::Port).map_err(|e| e.into())
            }
            Some(_) => Err(Error(ErrKind::RightInvalidChar, host_or_path.short_len())),
            None => Ok(OptionalPortOrTag::none()),
        }?;
        Ok(Self {
            host_or_path,
            optional_port_or_tag: port_or_tag,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::span::SpanMethods;
    fn should_parse(src: &str, left: &str, right: &str) {
        let tag = DomainOrRefSpan::new(src);
        match tag {
            Ok(span) => {
                let consumed = span.host_or_path.span_of(src);
                assert_eq!(consumed, left);
                let rest = &src[consumed.len()..];
                assert_eq!(rest, right);
                // assert_eq!(
                //     span.optional_port_or_tag.span_of(&src[consumed.len()..]),
                //     right
                // );
                assert_eq!(
                    span.len(),
                    src.len(),
                    "span.len() == {} != src.len() == {}",
                    span.len(),
                    src.len()
                );
                // assert_eq!(src, consumed, "consumed only {consumed:?} of {src:?}");
            }
            Err(e) => {
                let index = e.index() as usize;
                assert!(
                    index < src.len(),
                    "error {:?} @ index {index} is greater than src.len() {}",
                    e.kind(),
                    src.len()
                );
                panic!("{src:?} -> {e:?} :: {:?}", src.as_bytes()[index] as char);
            }
        }
    }

    #[test]
    fn test_ambiguous() {
        should_parse("test.com:tag", "test.com", ":tag");
        should_parse("test_com", "test_com", "");
    }
}
