//! # Ipv6
//! Parser for non-Ipv4-mapped Ipv6 addresses.

// 42 is the max length for a restricted ipv6 address:
// group  1___2____3____4____5____6____7____8____
// ip     1234:6789:1234:6789:1234:6789:1235:6789
//        0   0    1    1    2    2    3    3   3
//        1   5    0    5    0    5    0    5   9

use core::num::NonZeroU8;

use crate::{
    err,
    span::{impl_span_methods_on_tuple, nonzero, Lengthy, OptionallyZero, ShortLength},
};

type Error = err::Error<u8>;

/// recognize an IPv6 address as defined in [RFC 3986](https://www.rfc-editor.org/rfc/rfc3986#appendix-A)
/// and then subsequently restricted by distribution/reference:
/// > ipv6address are enclosed between square brackets and may be represented
/// > in many ways, see rfc5952. Only IPv6 in compressed or uncompressed format
/// > are allowed, IPv6 zone identifiers (rfc6874) or Special addresses such as
/// > IPv4-Mapped are deliberately excluded.
/// > ```go
/// > ipv6address = `\[(?:[a-fA-F0-9:]+)\]`
/// > ```
/// > -- [github.com/distribution/reference][dist]
///
/// [dist]: https://github.com/distribution/reference/blob/main/regexp.go#L87-90
#[derive(Clone, Copy)]
pub(crate) struct Ipv6Span<'src>(ShortLength<'src>);
impl_span_methods_on_tuple!(Ipv6Span, u8, NonZeroU8);

struct State(u8);
impl State {
    /// 0b00000111 possible values ina 0-7: 8 0-indexed groups
    const CURRENT_GROUP: u8 = 0b0111;
    /// 0b00011000 possible values ina 0-3: 4 0-indexed positions
    const POSITION_IN_GROUP: u8 = 3 << 3;
    /// 0b01100000 possible values in 0-3: 3 1-indexed colons
    const COLON_COUNT: u8 = 0b0011 << 5;
    /// 0b10000000 possible values in 0-1: bool
    const DOUBLE_COLON: u8 = 1 << 7;

    // setters -------------------------------------------------------------
    fn increment_position_in_group(&mut self) -> Result<(), err::Kind> {
        self.set_colon_count(0)?;
        let pos = if self.last_was_colon() {
            self.position_in_group().saturating_add(1)
        } else {
            0
        };
        self.set_position_in_group(pos) // checks for overflow of max possible position
    }
    fn set_colon_count(&mut self, count: u8) -> Result<(), err::Kind> {
        match count {
            0 => self.set_last_was_colon(false),
            1 => self.set_last_was_colon(true),
            2 => {
                self.set_last_was_colon(true);
                self.set_double_colon()?;
            }
            _ => return Err(err::Kind::Ipv6BadColon),
        };

        // update the colon count
        self.0 &= !Self::COLON_COUNT; // clear the colon count
        self.0 |= count << 5;

        Ok(())
    }
    fn increment_colon_count(&mut self) -> Result<(), err::Kind> {
        self.set_colon_count(self.colon_count().saturating_add(1))
    }
    fn set_position_in_group(&mut self, pos: u8) -> Result<(), err::Kind> {
        match pos {
            0..=3 => {
                self.0 &= !Self::POSITION_IN_GROUP; // clear the position in group
                self.0 |= pos << 3; // update the position in group
                Ok(())
            }
            _ => Err(err::Kind::Ipv6TooManyHexDigits),
        }
    }
    fn set_group(&mut self, group: u8) -> Result<(), err::Kind> {
        match group {
            0..=7 => {
                self.0 &= !Self::CURRENT_GROUP; // clear the current group
                self.0 |= group & Self::CURRENT_GROUP;
                Ok(()) // update the current group
            }
            _ => Err(err::Kind::Ipv6TooManyGroups),
        }
    }
    fn increment_group(&mut self) -> Result<(), err::Kind> {
        self.set_group(self.current_group().saturating_add(1))
    }
    fn set_colon(&mut self) -> Result<(), err::Kind> {
        self.increment_colon_count()?;
        self.increment_group()?;
        self.set_position_in_group(0) // <- position=0 is always valid
    }
    fn set_double_colon(&mut self) -> Result<(), err::Kind> {
        if self.double_colon_already_seen() {
            Err(err::Kind::Ipv6BadColon)
        } else {
            self.0 |= Self::DOUBLE_COLON;
            Ok(())
        }
    }
    fn set_last_was_colon(&mut self, last_was_colon: bool) {
        self.0 |= (if last_was_colon { 1 } else { 0 }) << 5;
    }
    // getters -------------------------------------------------------------
    /// returns the group index, 0-7.
    const fn current_group(&self) -> u8 {
        self.0 & Self::CURRENT_GROUP
    }
    const fn double_colon_already_seen(&self) -> bool {
        (self.0 & Self::DOUBLE_COLON) == Self::DOUBLE_COLON
    }
    const fn last_was_colon(&self) -> bool {
        self.colon_count() > 0
    }
    const fn position_in_group(&self) -> u8 {
        (self.0 & Self::POSITION_IN_GROUP) >> 3
    }
    // max value: 3
    const fn colon_count(&self) -> u8 {
        (self.0 & Self::COLON_COUNT) >> 5_u8
    }
}
impl<'src> Ipv6Span<'src> {
    pub(crate) fn new(src: &'src str) -> Result<Self, Error> {
        let mut ascii = src.bytes();
        let mut index: NonZeroU8 = match ascii.next() {
            None => Error::at(0, err::Kind::HostMissing).into(),
            Some(b'[') => Ok(nonzero!(u8, 1_u8)), // consume the opening bracket
            Some(_) => Error::at(0, err::Kind::Ipv6InvalidChar).into(),
        }?;
        let mut state = State(0);
        loop {
            // loop until we reach the closing bracket or encounter an error
            if let Some(next) = ascii.next() {
                match next {
                    b'a'..=b'f' | b'A'..=b'F' | b'0'..=b'9' => state.increment_position_in_group(),
                    b':' => state.set_colon(),
                    b']' => break, // done!
                    b'/' => Err(err::Kind::Ipv6MissingClosingBracket),
                    _ => Err(err::Kind::Ipv6InvalidChar),
                }
                .map_err(|kind| Error::at(index.upcast(), kind))?;
            } else {
                return Err(Error::at(
                    index.upcast(),
                    err::Kind::Ipv6MissingClosingBracket,
                ));
            };
            index = index
                .checked_add(1)
                .ok_or(Error::at(u8::MAX, err::Kind::Ipv6TooLong))?;
        }
        debug_assert!(src.as_bytes().first() == Some(&b'['));
        debug_assert!(src.as_bytes().get(index.as_usize()) == Some(&b']'));
        index = index
            .checked_add(1) // consume t he closing bracket
            .ok_or(Error::at(u8::MAX, err::Kind::Ipv6TooLong))?;
        match state.current_group() {
            0..=6 => {
                if state.double_colon_already_seen() {
                    Ok(Self(ShortLength::from_nonzero(index)))
                } else {
                    Err(Error::at(index.upcast(), err::Kind::Ipv6TooFewGroups))
                }
            }
            7 => Ok(Self(ShortLength::from_nonzero(index))),
            _ => unreachable!(), // group_count <= 7 enforced by checks on state.increment_group()
        }
    }
}

#[cfg(test)]
mod test {
    use crate::span::Lengthy;
    #[allow(clippy::indexing_slicing)]
    fn should_work(ip: &str) {
        match super::Ipv6Span::new(ip) {
            Ok(span) => assert_eq!(
                span.span_of(ip),
                ip,
                "\n\tparsed: {:?}\n\tip    : {ip:?}",
                span.span_of(ip),
            ),
            Err(e) => panic!(
                "failed to parse\n{ip}\n{}^\n{:?} @ {}",
                &ip[0..e.index() as usize + 1],
                e.kind(),
                e.index(),
            ),
        }
    }
    fn should_fail(ip: &str) {
        if let Ok(span) = super::Ipv6Span::new(ip) {
            panic!("should have failed to parse\n{ip}\n{}", span.span_of(ip),)
        }
    }

    #[test]
    fn test_no_ipv4_mapped() {
        should_fail("[0:0:0:0:127.0.0.1]");
    }
    #[test]
    fn test_parsing_valid_ips() {
        for ip in include_str!("./valid_ipv6.tsv")
            .split('\n')
            .filter(|s| !s.is_empty())
        {
            should_work(ip)
        }
    }
}
