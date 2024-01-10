use crate::{
    domain::ipv6,
    span::{impl_span_methods_on_tuple, IntoOption, OptionalSpan, Span, U},
};

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum AmbiguousErrorKind {
    // could apply to host or path ---------------------------------------------
    NoMatch,
    TooLong,
    InvalidChar,
    // Ipv6-specific -----------------------------------------------------------
    Ipv6NoMatch,
    Ipv6TooLong,
    Ipv6BadColon,
    Ipv6TooManyHexDigits,
    Ipv6TooManyGroups,
    Ipv6TooFewGroups,
    Ipv6MissingClosingBracket,
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Error(AmbiguousErrorKind, U);
impl std::ops::Add<U> for Error {
    type Output = Self;
    fn add(self, rhs: U) -> Self {
        Self(self.0, self.1 + rhs)
    }
}
impl Error {
    pub(crate) fn kind(&self) -> AmbiguousErrorKind {
        self.0
    }
    pub(crate) fn index(&self) -> U {
        self.1
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub(crate) enum Kind {
    /// could be either a host or a path
    Either = 0,
    /// a path component that's incompatible with being a hostname, i.e. it
    /// contains underscore(s).
    Path = Scan::HAS_UNDERSCORE,
    /// a hostname component that's incompatible with being a path. This variant means
    /// the span contains uppercase letter(s), or is immediately followed by a '/'.
    Host = Scan::HAS_UPPERCASE,
    /// Not ambiguous: an IPv6 address wrapped in square brackets, e.g. "[2001:db8::1]"
    IpV6 = Scan::INVALID,
}
impl From<Kind> for Scan {
    fn from(kind: Kind) -> Self {
        Self(kind as u8)
    }
}

const MASK: u8 = Kind::Path as u8 | Kind::Host as u8;
impl From<Scan> for Kind {
    fn from(scan: Scan) -> Self {
        // TODO: compare performance of match vs if-else with inlined bit-ops
        let raw = scan.0 & MASK;
        match raw {
            0 => Self::Either,
            Scan::HAS_UNDERSCORE => Self::Path,
            Scan::HAS_UPPERCASE => Self::Host,
            _ => unreachable!("incompatible flags set: {raw:b}"),
        }
    }
}
struct Scan(u8);
impl Scan {
    /// the number of underscores
    const UNDERSCORE_COUNT: u8 = 0b0011; //     0b00000011;
    const HAS_UNDERSCORE: u8 = 1 << 2; //       0b00000100;
    const HAS_UPPERCASE: u8 = 1 << 3; //        0b00001000;
    const LAST_WAS_DOT: u8 = 1 << 4; //         0b00010000;
    const LAST_WAS_DASH: u8 = 1 << 5; //        0b00100000;
    const INVALID: u8 = 1 << 6; //              0b01000000;

    // setters -----------------------------------------------------------------
    // all of which are fallible
    fn set_dot(&mut self) -> Result<(), Error> {
        if self.last_was_dot() {
            Err(Error(AmbiguousErrorKind::InvalidChar, 0))
        } else {
            Ok(self.0 |= Self::LAST_WAS_DOT)
        }
    }

    fn set_dash(&mut self) -> Result<(), Error> {
        if self.last_was_dash() {
            Err(Error(AmbiguousErrorKind::InvalidChar, 0))
        } else {
            Ok(self.0 |= Self::LAST_WAS_DASH)
        }
    }

    fn set_upper(&mut self) -> Result<(), Error> {
        if self.has_underscore() {
            Err(Error(AmbiguousErrorKind::InvalidChar, 0))
        } else {
            self.reset();
            Ok(self.0 |= Self::HAS_UPPERCASE)
        }
    }
    fn set_underscore_count(&mut self, count: u8) -> Result<(), Error> {
        if count > 2 {
            Err(Error(AmbiguousErrorKind::InvalidChar, 0))
        } else {
            self.0 &= !Self::UNDERSCORE_COUNT; // clear the count
            Ok(self.0 |= count)
        }
    }
    fn add_underscore(&mut self) -> Result<(), Error> {
        if self.has_upper() {
            Err(Error(AmbiguousErrorKind::InvalidChar, 0))
        } else if self.last_was_dash() || self.last_was_dot() {
            // TODO: use more specific error kind?
            Err(Error(AmbiguousErrorKind::InvalidChar, 0))
        } else {
            self.0 |= Self::HAS_UNDERSCORE;
            self.set_underscore_count(self.underscore_count() + 1)
        }
    }

    // resetters ---------------------------------------------------------------
    // all of these are infallible
    fn reset(&mut self) {
        self.reset_underscore();
        self.unset_dash();
        self.unset_dot();
    }
    fn unset_dot(&mut self) {
        self.0 &= !Self::LAST_WAS_DOT;
    }
    fn unset_dash(&mut self) {
        self.0 &= !Self::LAST_WAS_DASH;
    }

    fn reset_underscore(&mut self) {
        self.set_underscore_count(0).unwrap()
    }

    // getters -----------------------------------------------------------------
    fn has_upper(&self) -> bool {
        self.0 & Self::HAS_UPPERCASE == Self::HAS_UPPERCASE
    }
    fn last_was_dot(&self) -> bool {
        self.0 & Self::LAST_WAS_DOT == Self::LAST_WAS_DOT
    }
    fn last_was_dash(&self) -> bool {
        self.0 & Self::LAST_WAS_DASH == Self::LAST_WAS_DASH
    }
    fn underscore_count(&self) -> u8 {
        self.0 & Self::UNDERSCORE_COUNT
    }
    fn has_underscore(&self) -> bool {
        self.0 & Self::HAS_UNDERSCORE == Self::HAS_UNDERSCORE
    }
}

#[cfg(test)]
struct DebugScan {
    #[allow(dead_code)]
    has_upper: bool,
    #[allow(dead_code)]
    last_was_dot: bool,
    #[allow(dead_code)]
    last_was_dash: bool,
    #[allow(dead_code)]
    underscore_count: u8,
    #[allow(dead_code)]
    has_underscore: bool,
}

#[cfg(test)]
impl From<&Scan> for DebugScan {
    fn from(scan: &Scan) -> Self {
        Self {
            has_upper: scan.has_upper(),
            last_was_dot: scan.last_was_dot(),
            last_was_dash: scan.last_was_dash(),
            underscore_count: scan.underscore_count(),
            has_underscore: scan.has_underscore(),
        }
    }
}

// FIXME: use Span<'src> instead of OptionalSpan<'src>
#[derive(Clone, Copy)]
pub(crate) struct OptionalHostOrPath<'src>(Span<'src>, Kind);
impl_span_methods_on_tuple!(OptionalHostOrPath);
impl<'src> IntoOption for OptionalHostOrPath<'src> {
    fn is_some(&self) -> bool {
        self.short_len() > 0
    }
    fn none() -> Self
    where
        Self: Sized,
    {
        Self(Span::new(0), Kind::Either)
    }
}
impl OptionalHostOrPath<'_> {
    pub(super) fn narrow(self, kind: Kind) -> Result<Self, Error> {
        // TODO: consider moving this fn into DomainOrRef
        use Kind::*;
        match (self.kind(), kind) {
            (_, Either) => panic!("cannot narrow to Either"),
            (Either, _) => Ok(Self(self.0, kind)),
            (IpV6, IpV6) | (Path, Path) | (Host, Host) => Ok(self),
            (_, IpV6) => Err(Error(AmbiguousErrorKind::InvalidChar, 0)),
            (_, Path) => Err(Error(AmbiguousErrorKind::InvalidChar, 0)), // FIXME: find the underscore(s)
            (_, Host) => Err(Error(AmbiguousErrorKind::InvalidChar, 0)), // FIXME: find the uppercase letter(s)
        }
    }
    #[inline(always)]
    pub(crate) fn kind(&self) -> Kind {
        self.1
    }
    fn from_ipv6(src: &str) -> Result<Self, Error> {
        use crate::err;
        let span = ipv6::Ipv6Span::new(src.as_bytes()).map_err(|e| match e.0 {
            err::Kind::Ipv6BadColon => Error(AmbiguousErrorKind::Ipv6BadColon, e.1),
            err::Kind::Ipv6NoMatch => Error(AmbiguousErrorKind::Ipv6NoMatch, e.1),
            err::Kind::Ipv6TooLong => Error(AmbiguousErrorKind::Ipv6TooLong, e.1),
            err::Kind::Ipv6TooManyHexDigits => Error(AmbiguousErrorKind::Ipv6TooManyHexDigits, e.1),
            err::Kind::Ipv6TooManyGroups => Error(AmbiguousErrorKind::Ipv6TooManyGroups, e.1),
            err::Kind::Ipv6TooFewGroups => Error(AmbiguousErrorKind::Ipv6TooFewGroups, e.1),
            err::Kind::Ipv6MissingClosingBracket => {
                Error(AmbiguousErrorKind::Ipv6MissingClosingBracket, e.1)
            }
            _ => unreachable!(),
        })?;
        Ok(Self(span.span().into(), Kind::IpV6))
    }

    pub(crate) fn new(src: &str, kind: Kind) -> Result<Self, Error> {
        let ascii = src.as_bytes();
        let mut scan: Scan = kind.into(); // <- scan's setters will enforce the kind's constraint(s)
        let mut index = match ascii.iter().next() {
            None => Err(Error(AmbiguousErrorKind::NoMatch, 0)),
            Some(b'a'..=b'z') | Some(b'0'..=b'9') => Ok(0),
            Some(b'A'..=b'Z') => {
                scan.set_upper()?;
                Ok(0)
            }
            Some(b'[') => {
                // TODO: ensure this is unreachable
                return match kind {
                    Kind::Either | Kind::IpV6 => Self::from_ipv6(src),
                    _ => Err(Error(AmbiguousErrorKind::InvalidChar, 0)),
                };
            }
            Some(_) => Err(Error(AmbiguousErrorKind::InvalidChar, 0)),
        }?;

        while (index as usize) < ascii.len() - 1 {
            index += if index < U::MAX {
                Ok(1)
            } else {
                Err(Error(AmbiguousErrorKind::TooLong, index))
            }?;
            let c = ascii[index as usize];
            #[cfg(test)]
            let (_ch, _pre) = (c as char, DebugScan::from(&scan));
            match c {
                b'a'..=b'z' | b'0'..=b'9' => Ok(scan.reset()),
                b'A'..=b'Z' => scan.set_upper(),
                b'_' => scan.add_underscore(),
                b'.' => scan.set_dot(),
                b'-' => scan.set_dash(),
                b':' | b'/' => {
                    index -= 1;
                    break;
                }
                _ => Err(Error(AmbiguousErrorKind::InvalidChar, index)),
            }
            .map_err(|e| e + index)?;
            #[cfg(test)]
            {
                let _post = DebugScan::from(&scan);
                debug_assert!(
                    _pre.has_upper == _post.has_upper,
                    "has_upper changed from {} to {} @ {}",
                    _pre.has_upper,
                    _post.has_upper,
                    index
                );
            }
        }
        if scan.last_was_dash() || scan.last_was_dot() || scan.underscore_count() > 0 {
            Err(Error(AmbiguousErrorKind::InvalidChar, index))?;
        }
        debug_assert!(
            index < src.len() as u8,
            "index = {}, src.len() = {}",
            index,
            src.len()
        );
        Ok(Self(Span::new(index + 1), scan.into()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::span::SpanMethods;
    fn should_parse_as(src: &str, kind: Kind) {
        let host_or_path = should_parse(src);
        let _ = host_or_path.span_of(src); // check bounds
        assert_eq!(host_or_path.kind(), kind, "incorrectly typed {src:?}");
    }
    fn should_parse_incomplete(src: &str, expected: &str) {
        let host_or_path = should_parse(src);
        let consumed = host_or_path.span_of(src);
        let rest = &src[consumed.len()..];
        assert_eq!(
            rest, expected,
            "incorrectly left {rest:?} instead of {expected:?} of {src:?}"
        );
    }

    fn should_parse(src: &str) -> super::OptionalHostOrPath<'_> {
        super::OptionalHostOrPath::new(src, super::Kind::Either)
            .map_err(|e| {
                assert!(
                    false,
                    "failed to parse {:?}: {:?} @ {}",
                    src,
                    e.kind(),
                    e.index()
                );
            })
            .unwrap()
    }
    fn should_fail_with(src: &str, err_kind: super::AmbiguousErrorKind, bad_char_index: u8) {
        let err = super::OptionalHostOrPath::new(src, super::Kind::Either)
            .map(|e| {
                assert!(
                    false,
                    "should have failed to parse {:?}: {:?} @ {}",
                    src,
                    e.kind(),
                    e.len()
                );
            })
            .unwrap_err();
        assert_eq!(err.kind(), err_kind, "incorrect error kind");
        assert_eq!(
            err.index(),
            bad_char_index.into(),
            "expected offset of incorrect char to be {bad_char_index}; got {}",
            err.index()
        );
    }
    #[test]
    fn test_valid() {
        should_parse_as("example.com", Kind::Either);
        should_parse_as("example.com:tag", Kind::Either);
        should_parse_as("127.0.0.1", Kind::Either);
        should_parse_as("123.456.789.101", Kind::Either);
        should_parse_as("0.0", Kind::Either);
        should_parse_as("1.2.3.4.5", Kind::Either);
        should_parse_as("sub_domain.ex.com", Kind::Path);
        should_parse_as("Example.Com", Kind::Host);
    }
    #[test]
    fn test_stopping() {
        should_parse_incomplete("example.com:tag", ":tag");
        should_parse_incomplete("0.0.0.0:80", ":80");
        should_parse_incomplete("example.com/path", "/path");
        should_parse_incomplete("foo/bar", "/bar");
    }
    #[test]
    fn test_invalid() {
        should_fail_with(
            "google.com.",
            super::AmbiguousErrorKind::InvalidChar,
            ("google.com.".len() - 1) as u8,
        );
    }
}
