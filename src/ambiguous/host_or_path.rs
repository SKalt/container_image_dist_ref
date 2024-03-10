//! Hosts and paths can conflict:
//! | class | domain-component | path-component |
//! | ----- | ---------------- | -------------- |
//! | upper | yes              | no             |
//! | -     | inner            | inner          |
//! | _     | no               | inner          |
//! | .     | yes              | yes            |
//!
//! See the grammar:

// {{{sh sed 's#^#//! #g' ../../grammars/host_or_path.ebnf; printf '//! ```\n\n// ' }}}{{{out skip=2

//! ```ebnf
//! name                 ::= (domain "/")? path
//! domain               ::= host (":" port-number)?
//! host                 ::= domain-name | IPv4address | "[" IPv6address "]" /* see https://www.rfc-editor.org/rfc/rfc3986#appendix-A */
//! domain-name          ::= domain-component ("." domain-component)*
//! domain-component     ::= ([a-zA-Z0-9]|[a-zA-Z0-9][a-zA-Z0-9-]*[a-zA-Z0-9])
//! port-number          ::= [0-9]+
//! path-component       ::= [a-z0-9]+ (separator [a-z0-9]+)*
//! path                 ::= path-component ("/" path-component)*
//! separator            ::= [_.] | "__" | "-"+
//! ```

// }}}

use core::num::NonZeroU8;

use crate::{
    domain::ipv6,
    err::{
        self,
        Kind::{
            HostOrPathInvalidChar as InvalidChar, HostOrPathInvalidComponentEnd, HostOrPathTooLong,
        },
    },
    span::{impl_span_methods_on_tuple, Lengthy, ShortLength},
};

type Error = err::Error<u8>;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Kind {
    /// a path component that's incompatible with being a hostname, i.e. it
    /// contains underscore(s).
    Path,
    /// a hostname component that's incompatible with being a path. This variant means
    /// the span contains uppercase letter(s).
    Host,
    /// Not ambiguous: an IPv6 address wrapped in square brackets, e.g. "[2001:db8::1]"
    IpV6,
    /// could be either a path or a hostname since it contains neither underscores
    /// nor uppercase letters
    HostOrPath,
    Any,
}

impl From<Scan> for Kind {
    fn from(scan: Scan) -> Self {
        const ANY: u8 = Scan::IPV6 | Scan::HAS_UPPERCASE | Scan::HAS_UNDERSCORE;
        match scan.0 & ANY {
            0 => Self::HostOrPath,
            Scan::HAS_UPPERCASE => Self::Host,
            Scan::HAS_UNDERSCORE => Self::Path,
            _ => unreachable!(),
        }
    }
}

impl From<Kind> for Scan {
    fn from(kind: Kind) -> Self {
        match kind {
            Kind::Host => Self(Self::HAS_UPPERCASE),
            Kind::Path => Self(Self::HAS_UNDERSCORE),
            Kind::IpV6 => Self(Self::IPV6),
            Kind::HostOrPath | Kind::Any => Self(0),
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
    const IPV6: u8 = 1 << 6; //              0b01000000;

    // setters -----------------------------------------------------------------
    // all of which are fallible
    fn set_dot(&mut self) -> Result<(), err::Kind> {
        if self.last_was_dot() || self.last_was_dash() || self.underscore_count() > 0 {
            Err(HostOrPathInvalidComponentEnd)
        } else {
            self.0 |= Self::LAST_WAS_DOT;
            Ok(())
        }
    }

    fn set_dash(&mut self) -> Result<(), err::Kind> {
        if self.last_was_dot() || self.underscore_count() > 0 {
            Err(HostOrPathInvalidComponentEnd)
        } else {
            self.0 |= Self::LAST_WAS_DASH;
            Ok(())
        }
    }

    fn set_upper(&mut self) -> Result<(), err::Kind> {
        if self.has_underscore() {
            Err(InvalidChar)
        } else {
            self.0 |= Self::HAS_UPPERCASE;
            self.reset()
        }
    }
    fn set_underscore_count(&mut self, count: u8) -> Result<(), err::Kind> {
        match count {
            0..=2 => {
                self.0 &= !Self::UNDERSCORE_COUNT; // clear the count
                self.0 |= count; // update the count
                Ok(())
            }
            _ => Err(InvalidChar),
        }
    }
    fn add_underscore(&mut self) -> Result<(), err::Kind> {
        if self.has_upper() {
            Err(InvalidChar)
        } else if self.last_was_dash() || self.last_was_dot() {
            Err(HostOrPathInvalidComponentEnd)
        } else {
            self.0 |= Self::HAS_UNDERSCORE;
            self.set_underscore_count(self.underscore_count() + 1)
        }
    }

    // resetters ---------------------------------------------------------------
    // all of these are infallible
    fn reset(&mut self) -> Result<(), err::Kind> {
        // for convenience
        self.reset_underscore_count();
        self.unset_last_was_dash();
        self.unset_last_was_dot();
        Ok(())
    }
    fn unset_last_was_dot(&mut self) {
        self.0 &= !Self::LAST_WAS_DOT;
    }
    fn unset_last_was_dash(&mut self) {
        self.0 &= !Self::LAST_WAS_DASH;
    }

    fn reset_underscore_count(&mut self) {
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

struct State {
    len: u8,
    scan: Scan,
    deciding_char: Option<u8>,
}
impl State {
    fn advance(&mut self) -> Result<(), Error> {
        self.len = self
            .len
            .checked_add(1)
            .ok_or(Error::at(self.len, HostOrPathTooLong))?;
        Ok(())
    }
    fn update_decider(&mut self) {
        self.deciding_char = self.deciding_char.or(Some(self.len));
    }
    fn update(&mut self, ascii_char: u8) -> Result<(), Error> {
        #[cfg(debug_assertions)]
        let _c = ascii_char as char;

        match ascii_char {
            b'a'..=b'z' | b'0'..=b'9' => self.scan.reset(),
            b'A'..=b'Z' => self.scan.set_upper().map(|_| self.update_decider()),
            b'_' => self.scan.add_underscore().map(|_| self.update_decider()),
            b'.' => self.scan.set_dot(),
            b'-' => self.scan.set_dash(),
            _ => Err(InvalidChar),
        }
        .map_err(|err_kind| Error::at(self.len, err_kind))
    }
    fn check_component_end(&self) -> Result<(), Error> {
        let ok = !self.scan.last_was_dash()
            && !self.scan.last_was_dot()
            && self.scan.underscore_count() == 0;
        match ok {
            true => Ok(()),
            false => Err(Error::at(self.len - 1, HostOrPathInvalidComponentEnd)),
        }
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
pub(crate) struct HostOrPathSpan<'src>(ShortLength<'src>, Kind, u8);
impl_span_methods_on_tuple!(HostOrPathSpan, u8, NonZeroU8);

impl<'src> HostOrPathSpan<'src> {
    pub(crate) fn kind(&self) -> Kind {
        self.1
    }

    /// can return None if at EOF or the first character is a `/` or `@`
    pub(crate) fn new(src: &'src str, kind: Kind) -> Result<Self, Error> {
        let mut state = State {
            len: 0,
            scan: kind.into(), // <- scan's setters will enforce the kind's constraint(s)
            deciding_char: None,
        };
        {
            // check the first character, if any
            let c = src.bytes().next();
            #[cfg(test)]
            let _c = c.map(|c| c as char);
            match c {
                None => return Error::at(0, err::Kind::HostOrPathMissing).into(),
                Some(b'[') => {
                    return match kind {
                        Kind::IpV6 | Kind::Any => {
                            let span = ipv6::Ipv6Span::new(src)?;
                            Ok(Self(span.into_length().unwrap(), Kind::IpV6, 0))
                        }
                        _ => Err(Error::at(0, InvalidChar)),
                    }
                }
                Some(b'.') | Some(b'-') | Some(b'_') => return Error::at(0, InvalidChar).into(),
                _ => {}
            };
        };

        for c in src.bytes() {
            #[cfg(debug_assertions)]
            let (_pre, _ch) = (DebugScan::from(&state.scan), c as char);
            match c {
                b':' | b'/' | b'@' => break, // done!
                _ => state.update(c),
            }?;
            #[cfg(debug_assertions)]
            let _post = DebugScan::from(&state.scan);
            state.advance()?;
        }
        #[cfg(debug_assertions)]
        let _done = DebugScan::from(&state.scan);

        state.check_component_end()?;
        #[cfg(debug_assertions)]
        let _rest = &src[state.len as usize..];
        debug_assert!(
            state.len as usize <= src.len(),
            "len = {}, src.len() = {} for {src:?}",
            state.len,
            src.len()
        );
        ShortLength::new(state.len)
            .ok_or(Error::at(0, err::Kind::HostOrPathMissing))
            .map(|length| Self(length, state.scan.into(), state.deciding_char.unwrap_or(0)))
    }
    pub(crate) fn narrow(self, target_kind: Kind) -> Result<Self, Error> {
        use Kind::*;
        let decider = self.2;
        match (self.kind(), target_kind) {
            (_, Any) => unreachable!("calls to .narrow() must narrow, not broaden"),
            (Path, HostOrPath) | (Host, HostOrPath) => Ok(self),
            (Any, _) | (HostOrPath, Path) | (HostOrPath, Host) => {
                Ok(Self(self.0, target_kind, decider))
            }
            (IpV6, IpV6) | (Path, Path) | (Host, Host) | (HostOrPath, HostOrPath) => Ok(self),
            (_, IpV6) | (IpV6, _) => Error::at(0, InvalidChar).into(),
            (Host, Path) => Error::at(decider, err::Kind::PathInvalidChar).into(),
            (Path, Host) => Error::at(decider, err::Kind::HostInvalidChar).into(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::span::Lengthy;
    fn should_parse(src: &str) -> super::HostOrPathSpan<'_> {
        HostOrPathSpan::new(src, Kind::Any)
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
        let output_kind = host_or_path.kind();
        assert_eq!(output_kind, kind, "incorrectly typed {src:?} as {kind:?}");
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
        let err = super::HostOrPathSpan::new(src, Kind::Any)
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
            bad_char_index,
            "expected index of incorrect char to be {bad_char_index} in {src:?}; got {}, which is the index of {:?}",
            err.index(),
            src.as_bytes()[err.index() as usize] as char
        );
    }
    #[test]
    fn test_valid() {
        // should_parse_as("example.com", Kind::Either);
        // should_parse_as("example.com:tag", Kind::Either);
        use Kind::*;
        should_parse_as("127.0.0.1", "127.0.0.1", HostOrPath);
        should_parse_as("123.456.789.101", "123.456.789.101", HostOrPath);
        should_parse_as("0.0", "0.0", HostOrPath);
        should_parse_as("1.2.3.4.5", "1.2.3.4.5", HostOrPath);
        should_parse_as("sub_domain.ex.com", "sub_domain.ex.com", Path.into());
        should_parse_as("Example.Com", "Example.Com", Host.into());
        should_parse_as("example.com:tag", "example.com", HostOrPath);
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
            HostOrPathInvalidComponentEnd,
            ("google.com.".len() - 1) as u8,
        );
    }
}
