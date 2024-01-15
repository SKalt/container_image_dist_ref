use crate::{
    domain::ipv6,
    err::{self, Error},
    span::{impl_span_methods_on_tuple, IntoOption, Lengthy, Short, ShortLength},
};
use err::Kind::{
    HostOrPathInvalidChar as InvalidChar, HostOrPathInvalidComponentEnd as InvalidComponentEnd,
    HostOrPathNoMatch as NoMatch, HostOrPathTooLong as TooLong,
};

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
    fn set_dot(&mut self) -> Result<(), err::Kind> {
        if self.last_was_dot() || self.last_was_dash() {
            Err(InvalidComponentEnd)
        } else {
            self.0 |= Self::LAST_WAS_DOT;
            Ok(())
        }
    }

    fn set_dash(&mut self) -> Result<(), err::Kind> {
        if self.last_was_dot() || self.underscore_count() > 0 {
            Err(InvalidComponentEnd)
        } else {
            self.0 |= Self::LAST_WAS_DASH;
            Ok(())
        }
    }

    fn set_upper(&mut self) -> Result<(), err::Kind> {
        if self.has_underscore() {
            Err(InvalidChar)
        } else {
            self.reset();
            self.0 |= Self::HAS_UPPERCASE;
            Ok(())
        }
    }
    fn set_underscore_count(&mut self, count: u8) -> Result<(), err::Kind> {
        if count > 2 {
            Err(InvalidChar)
        } else {
            self.0 &= !Self::UNDERSCORE_COUNT; // clear the count
            self.0 |= count; // update the count
            Ok(())
        }
    }
    fn add_underscore(&mut self) -> Result<(), err::Kind> {
        if self.has_upper() {
            Err(InvalidChar)
        } else if self.last_was_dash() || self.last_was_dot() {
            Err(InvalidComponentEnd)
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

#[cfg(debug_assertions)]
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

#[cfg(debug_assertions)]
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

#[derive(Clone, Copy)]
pub(crate) struct HostOrPathSpan<'src>(ShortLength<'src>, Kind);
impl_span_methods_on_tuple!(HostOrPathSpan, Short);
impl<'src> IntoOption for HostOrPathSpan<'src> {
    fn is_some(&self) -> bool {
        self.short_len() > 0
    }
    fn none() -> Self {
        Self(0.into(), Kind::Either)
    }
}
impl<'src> HostOrPathSpan<'src> {
    pub(crate) fn narrow(self, target_kind: Kind, context: &'src str) -> Result<Self, Error> {
        use Kind::*;
        match (self.kind(), target_kind) {
            (_, Either) => {
                debug_assert!(false, "don't narrow to Either, that's broadening");
                Ok(Self(self.0, Either))
            }
            (Either, _) => Ok(Self(self.0, target_kind)),
            (IpV6, IpV6) | (Path, Path) | (Host, Host) => Ok(self),
            (_, IpV6) => Error::at(0, InvalidChar).into(),
            (IpV6, Path) | (IpV6, Host) => Error::at(0, InvalidChar).into(), // 0 must be an opening [, which is invalid for a Host or Path
            (Host, Path) => {
                let i = {
                    let underscore_index = self
                        .span_of(context)
                        .bytes()
                        .enumerate()
                        .find(|(_, b)| b.is_ascii_uppercase())
                        .map(|(i, _)| i);
                    debug_assert!(
                        underscore_index.is_some(),
                        "unable to find _ in {context:?}"
                    );
                    underscore_index.unwrap() // safe since this self.kind() == Host means there must have been an uppercase letter
                };
                let i = i.try_into().unwrap(); // safe since self.span_of(context) must be short
                Error::at(i, err::Kind::PathInvalidChar).into()
            }
            (Path, Host) => {
                let offending_uppercase_index = self.span_of(context)
                    .bytes().enumerate()
                    .find(|(_, b)| b == &b'_')
                    .map(|(i, _)| i)
                    .unwrap() // safe since this self.kind() == Path means there must have been an underscore
                    .try_into()
                    .unwrap();
                Err(Error(offending_uppercase_index, err::Kind::HostInvalidChar))
            }
        }
    }
    pub(crate) fn kind(&self) -> Kind {
        self.1
    }
    fn from_ipv6(src: &'src str) -> Result<Self, Error> {
        let span = ipv6::Ipv6Span::new(src.as_bytes())?;
        Ok(Self(span.short_len().into(), Kind::IpV6))
    }

    pub(crate) fn new(src: &'src str, kind: Kind) -> Result<Self, Error> {
        if src.is_empty() {
            return Error::at(0, NoMatch).into();
        }
        let mut len = 0;
        let ascii = src.as_bytes();
        let mut scan: Scan = kind.into(); // <- scan's setters will enforce the kind's constraint(s)
        let c = ascii[len as usize];
        #[cfg(test)]
        let _c = c as char;
        len += match c {
            // safe since len is going from 0 -> 1
            b'a'..=b'z' | b'0'..=b'9' => Ok(1),
            b'A'..=b'Z' => {
                scan.set_upper().map_err(|kind| Error(0, kind))?;
                Ok(1)
            }
            b'[' => {
                return match kind {
                    Kind::Either | Kind::IpV6 => Self::from_ipv6(src),
                    _ => Error::at(0, InvalidChar).into(),
                };
            }
            _ => Err(Error(0, InvalidChar)),
        }?;

        while (len as usize) < ascii.len() {
            let c = ascii[len as usize];
            #[cfg(debug_assertions)]
            let (_c, _pre) = (c as char, DebugScan::from(&scan));
            match c {
                b'a'..=b'z' | b'0'..=b'9' => {
                    scan.reset();
                    Ok(())
                }
                b'A'..=b'Z' => scan.set_upper(),
                b'_' => scan.add_underscore(),
                b'.' => scan.set_dot(),
                b'-' => scan.set_dash(),
                b':' | b'/' | b'@' => break,
                _ => Err(InvalidChar),
            }
            .map_err(|err_kind| Error::at(len, err_kind))?;
            #[cfg(debug_assertions)]
            {
                let _post = DebugScan::from(&scan);
                if _c == '.' {
                    debug_assert!(
                        !_pre.last_was_dot && _post.last_was_dot,
                        "last_was_dot changed from {} to {} @ {}",
                        _pre.last_was_dot,
                        _post.last_was_dot,
                        len
                    );
                } else {
                    debug_assert!(!_post.last_was_dot)
                }
            }
            if len == Short::MAX {
                match ascii[len as usize..].iter().next() {
                    Some(b':') | Some(b'@') => break,
                    _ => return Error::at(len, TooLong).into(),
                };
            }
            len += 1; // safe due to the above length-guard
        }

        #[cfg(debug_assertions)]
        let _scan = DebugScan::from(&scan);
        if scan.last_was_dash() || scan.last_was_dot() || scan.underscore_count() > 0 {
            Err(Error(len - 1, err::Kind::HostOrPathInvalidComponentEnd))?;
        }
        debug_assert!(
            len as usize <= src.len(),
            "len = {len}, src.len() = {} for {src:?}",
            src.len()
        );
        Ok(Self(len.into(), scan.into()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::span::Lengthy;
    fn should_parse(src: &str) -> super::HostOrPathSpan<'_> {
        super::HostOrPathSpan::new(src, super::Kind::Either)
            .map_err(|e| {
                assert!(
                    false,
                    "failed to parse {:?}: {:?} @ {} ({:?})",
                    src,
                    e.kind(),
                    e.index(),
                    src.as_bytes()[e.index() as usize] as char
                );
            })
            .unwrap()
    }
    fn should_parse_as(src: &str, expected: &str, kind: Kind) {
        let host_or_path = should_parse(src);
        let consumed = host_or_path.span_of(src);
        assert_eq!(
            consumed, expected,
            "incorrectly parsed {consumed:?} of {src:?}"
        );
        assert_eq!(
            host_or_path.kind(),
            kind,
            "incorrectly typed {src:?} as {kind:?}"
        );
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

    fn should_fail_with(src: &str, err_kind: err::Kind, bad_char_index: u8) {
        let err = super::HostOrPathSpan::new(src, super::Kind::Either)
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
            "expected index of incorrect char to be {bad_char_index} in {src:?}; got {}, which is the index of {:?}",
            err.index(),
            src.as_bytes()[err.index() as usize] as char
        );
    }
    #[test]
    fn test_valid() {
        // should_parse_as("example.com", Kind::Either);
        // should_parse_as("example.com:tag", Kind::Either);
        should_parse_as("127.0.0.1", "127.0.0.1", Kind::Either);
        should_parse_as("123.456.789.101", "123.456.789.101", Kind::Either);
        should_parse_as("0.0", "0.0", Kind::Either);
        should_parse_as("1.2.3.4.5", "1.2.3.4.5", Kind::Either);
        should_parse_as("sub_domain.ex.com", "sub_domain.ex.com", Kind::Path);
        should_parse_as("Example.Com", "Example.Com", Kind::Host);
        should_parse_as("example.com:tag", "example.com", Kind::Either);
    }
    #[test]
    fn test_stopping() {
        should_parse_incomplete("example.com:tag", ":tag");
        should_parse_incomplete("0.0.0.0:80", ":80");
        should_parse_incomplete("example.com/path", "/path");
        should_parse_incomplete("foo/bar", "/bar");
        should_parse_incomplete("foo@algo:aaa", "@algo:aaa");
    }
    #[test]
    fn test_invalid() {
        should_fail_with("$", InvalidChar, 0);
        should_fail_with(
            "google.com.",
            InvalidComponentEnd,
            ("google.com.".len() - 1) as u8,
        );
    }
}
