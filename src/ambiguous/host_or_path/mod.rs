use crate::{
    domain::ipv6,
    span::{impl_span_methods_on_tuple, IntoOption, OptionalSpan, U},
};

#[derive(Clone, Copy)]
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
    pub(crate) fn len(&self) -> U {
        self.1
    }
}

#[derive(Clone, Copy, Debug)]
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
    const LAST_WAS_UNDERSCORE: u8 = 1 << 3; //  0b00001000;
    const HAS_UPPERCASE: u8 = 1 << 4; //        0b00010000;
    const LAST_WAS_DOT: u8 = 1 << 5; //         0b00100000;
    const LAST_WAS_DASH: u8 = 1 << 6; //        0b01000000;
    const INVALID: u8 = 1 << 7; //              0b10000000;
    fn new() -> Self {
        Self(0)
    }
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
        if self.caps_valid() {
            self.reset();
            Ok(self.0 |= Self::HAS_UPPERCASE)
        } else {
            Err(Error(AmbiguousErrorKind::InvalidChar, 0))
        }
    }
    fn add_underscore(&mut self) -> Result<(), Error> {
        if self.has_upper() {
            Err(Error(AmbiguousErrorKind::InvalidChar, 0))
        } else if self.last_was_dash() || self.last_was_dot() {
            // TODO: more error types
            Err(Error(AmbiguousErrorKind::InvalidChar, 0))
        } else {
            self.0 |= Self::HAS_UNDERSCORE;
            let mut underscore_count = self.underscore_count() + 1;
            if underscore_count > 2 {
                Err(Error(AmbiguousErrorKind::InvalidChar, 0))
            } else {
                self.unset_dash();
                self.unset_dot();
                Ok(self.0 |= underscore_count)
            }
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
        self.0 &= !Self::UNDERSCORE_COUNT;
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
    fn is_invalid(&self) -> bool {
        self.0 & Self::INVALID == Self::INVALID
    }
    // validity checks ---------------------------------------------------------
    fn underscore_valid(&self) -> bool {
        !self.has_upper() && self.valid_component_end()
    }
    fn caps_valid(&self) -> bool {
        !self.has_underscore()
    }

    pub(crate) fn valid_completed_path(&self) -> bool {
        !self.has_upper() && self.valid_component_end()
    }
    pub(crate) fn valid_completed_domain(&self) -> bool {
        !self.has_underscore() && self.valid_component_end()
    }
    pub(crate) fn valid_component_end(&self) -> bool {
        !self.last_was_dash() && !self.last_was_dot() && self.underscore_count() == 0
    }
    #[inline(always)]
    fn separator_valid(&self) -> bool {
        self.valid_component_end()
    }
}

#[derive(Clone, Copy)]
pub(crate) struct OptionalHostOrPath<'src>(OptionalSpan<'src>, Kind);
impl_span_methods_on_tuple!(OptionalHostOrPath);
impl<'src> IntoOption for OptionalHostOrPath<'src> {
    fn is_some(&self) -> bool {
        self.short_len() > 0
    }
    fn none() -> Self
    where
        Self: Sized,
    {
        Self(OptionalSpan::new(0), Kind::Either)
    }
}
impl OptionalHostOrPath<'_> {
    pub(super) fn narrow(self, kind: Kind) -> Result<Self, Error> {
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
        use crate::err; // FIXME: rename
        use AmbiguousErrorKind as A;
        let span = ipv6::Ipv6Span::new(src.as_bytes()).map_err(|e| match e.0 {
            err::Kind::Ipv6BadColon => Error(A::Ipv6BadColon, e.1),
            err::Kind::Ipv6NoMatch => Error(A::Ipv6NoMatch, e.1),
            err::Kind::Ipv6TooLong => Error(A::Ipv6TooLong, e.1),
            err::Kind::Ipv6TooManyHexDigits => Error(A::Ipv6TooManyHexDigits, e.1),
            err::Kind::Ipv6TooManyGroups => Error(A::Ipv6TooManyGroups, e.1),
            err::Kind::Ipv6TooFewGroups => Error(A::Ipv6TooFewGroups, e.1),
            err::Kind::Ipv6MissingClosingBracket => Error(A::Ipv6MissingClosingBracket, e.1),
            _ => unreachable!(),
        })?;
        Ok(Self(span.span().into(), Kind::IpV6))
    }

    pub(crate) fn new(src: &str, kind: Kind) -> Result<Self, Error> {
        let mut ascii = src.bytes();
        let mut scan: Scan = kind.into(); // <- scan's setters will enforce the kind's constraint(s)
        match ascii.next() {
            None => Err(Error(AmbiguousErrorKind::NoMatch, 0)),
            Some(b'a'..=b'z') | Some(b'0'..=b'9') => Ok(()),
            Some(b'A'..=b'Z') => scan.set_upper(),
            Some(b'[') => {
                // TODO: ensure this is unreachable
                return match kind {
                    Kind::Either | Kind::IpV6 => Self::from_ipv6(src),
                    _ => Err(Error(AmbiguousErrorKind::InvalidChar, 0)),
                };
            }
            Some(_) => Err(Error(AmbiguousErrorKind::InvalidChar, 0)),
        }?;
        let mut len = 1;

        for c in ascii {
            if len == U::MAX {
                Err(Error(AmbiguousErrorKind::TooLong, len))?;
            }
            match c {
                b'a'..=b'z' | b'0'..=b'9' => Ok(scan.reset()),
                b'A'..=b'Z' => scan.set_upper(),
                b'_' => scan.add_underscore(),
                b'.' => scan.set_dot(),
                b'-' => scan.set_dash(),
                b':' | b'/' => break,
                _ => Err(Error(AmbiguousErrorKind::InvalidChar, len)),
            }
            .map_err(|e| e + len)?
        }
        Ok(Self(OptionalSpan::new(len), kind))
    }
}
