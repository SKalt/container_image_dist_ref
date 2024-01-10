// 42 is the max length for a restricted ipv6 address:
// group  1___2____3____4____5____6____7____8____
// ip     1234:6789:1234:6789:1234:6789:1235:6789
//        0   0    1    1    2    2    3    3   3
//        1   5    0    5    0    5    0    5   9

use crate::{
    err::{Error, Kind as ErrorKind},
    span::impl_span_methods_on_tuple,
    Span, U,
};
/// recognize an IPv6 address as defined in https://www.rfc-editor.org/rfc/rfc3986#appendix-A
/// and then subsequently restricted by distribution/reference:
/// > ipv6address are enclosed between square brackets and may be represented
/// > in many ways, see rfc5952. Only IPv6 in compressed or uncompressed format
/// > are allowed, IPv6 zone identifiers (rfc6874) or Special addresses such as
/// > IPv4-Mapped are deliberately excluded.
/// > ```go
/// > ipv6address = `\[(?:[a-fA-F0-9:]+)\]`
/// > ```
/// > -- https://github.com/distribution/reference/blob/main/regexp.go#L87-90
#[derive(Clone, Copy)]
pub(crate) struct Ipv6Span<'src>(Span<'src>);
impl_span_methods_on_tuple!(Ipv6Span);

struct State(u8);
impl State {
    const CURRENT_GROUP: u8 = 0b0111; //     0b00000111 = 0-7 = 8 0-indexed groups
    const POSITION_IN_GROUP: u8 = 3 << 3; // 0b00011000 = 0-3 = 4 0-indexed positions
    const COLON_COUNT: u8 = 0b0011 << 5; //  0b01100000 = 0-3 = 3 1-indexed colons
    const DOUBLE_COLON: u8 = 1 << 7; //      0b10000000 = 0-1 = bool

    // setters -------------------------------------------------------------
    #[inline(always)]
    fn increment_position_in_group(&mut self) -> Result<(), Error> {
        let pos = if self.last_was_colon() {
            self.position_in_group() + 1
        } else {
            0
        };
        self.set_colon_count(0)?;
        self.set_position_in_group(pos)
    }
    #[inline(always)]
    fn set_colon_count(&mut self, count: u8) -> Result<(), Error> {
        match count {
            0 => Ok(self.set_last_was_colon(false)),
            1 => Ok(self.set_last_was_colon(true)),
            2 => {
                self.set_last_was_colon(true);
                self.set_double_colon()
            }
            _ => Err(Error(ErrorKind::Ipv6BadColon, 0)),
        }?;
        self.0 &= !Self::COLON_COUNT; // clear the colon count
        Ok(self.0 |= count << 5) // update the colon count
    }
    #[inline(always)]
    fn increment_colon_count(&mut self) -> Result<(), Error> {
        self.set_colon_count(self.colon_count() + 1)
    }
    #[inline(always)]
    fn set_position_in_group(&mut self, pos: u8) -> Result<(), Error> {
        match pos {
            0..=3 => {
                self.0 &= !Self::POSITION_IN_GROUP; // clear the position in group
                self.0 |= pos << 3; // update the position in group
                Ok(())
            }
            _ => Err(Error(ErrorKind::Ipv6TooManyHexDigits, 0)),
        }
    }
    #[inline(always)]
    fn set_group(&mut self, group: u8) -> Result<(), Error> {
        match group {
            0..=7 => {
                self.0 &= !Self::CURRENT_GROUP; // clear the current group
                Ok(self.0 |= group & Self::CURRENT_GROUP) // update the current group
            }
            _ => Err(Error(ErrorKind::Ipv6TooManyGroups, 0)),
        }
    }
    #[inline(always)]
    fn increment_group(&mut self) -> Result<(), Error> {
        self.set_group(self.current_group() + 1)
    }
    #[inline(always)]
    fn set_colon(&mut self) -> Result<(), Error> {
        self.increment_colon_count()?;
        self.increment_group()?;
        self.set_position_in_group(0) // <- position=0 is always valid
    }
    #[inline(always)]
    fn set_double_colon(&mut self) -> Result<(), Error> {
        if self.double_colon_already_seen() {
            Err(Error(ErrorKind::Ipv6BadColon, 0))
        } else {
            Ok(self.0 |= Self::DOUBLE_COLON)
        }
    }
    #[inline(always)]
    fn set_last_was_colon(&mut self, last_was_colon: bool) {
        self.0 |= (if last_was_colon { 1 } else { 0 }) << 5;
    }
    // getters -------------------------------------------------------------
    /// returns the group index, 0-7.
    #[inline(always)]
    fn current_group(&self) -> u8 {
        self.0 & Self::CURRENT_GROUP
    }
    #[inline(always)]
    fn double_colon_already_seen(&self) -> bool {
        (self.0 & Self::DOUBLE_COLON) == Self::DOUBLE_COLON
    }
    #[inline(always)]
    fn last_was_colon(&self) -> bool {
        self.colon_count() > 0
    }
    #[inline(always)]
    fn position_in_group(&self) -> u8 {
        (self.0 & Self::POSITION_IN_GROUP) >> 3
    }
    #[inline(always)]
    fn colon_count(&self) -> u8 {
        (self.0 & Self::COLON_COUNT) >> 5
    }
}
impl<'src> Ipv6Span<'src> {
    pub(crate) fn new(ascii_bytes: &[u8]) -> Result<Self, Error> {
        let mut index: U = if ascii_bytes.len() == 0 {
            Err(Error(ErrorKind::Ipv6NoMatch, 0))
        } else if ascii_bytes[0] != b'[' {
            Err(Error(ErrorKind::Ipv6NoMatch, 0))
        } else {
            Ok(0) // consume the opening bracket
        }?;
        let mut state = State(0);
        loop {
            index = if (ascii_bytes.len() - 1) == index as usize {
                Err(Error(ErrorKind::Ipv6MissingClosingBracket, index))
            } else if index == U::MAX {
                Err(Error(ErrorKind::Ipv6TooLong, index))
            } else {
                Ok(index + 1)
            }?;
            match ascii_bytes[index as usize] {
                b'a'..=b'f' | b'A'..=b'F' | b'0'..=b'9' => state.increment_position_in_group(),
                b':' => state.set_colon(),
                b']' => break, // done!
                _ => return Err(Error(ErrorKind::Ipv6MissingClosingBracket, 0)),
            }
            .map_err(|e| e + index)?;
        }
        debug_assert!(ascii_bytes[0] == b'[');
        debug_assert!(ascii_bytes[index as usize] == b']');
        let len = index + 1; // consume the closing bracket
        match state.current_group() {
            0..=6 => {
                if state.double_colon_already_seen() {
                    Ok(Self(Span::new(len)))
                } else {
                    Err(Error(ErrorKind::Ipv6TooFewGroups, index))
                }
            }
            7 => Ok(Self(Span::new(len))),
            _ => unreachable!("group_count <= 7 enforced by checks on state.increment_group()"),
        }
    }
    pub(crate) fn span(&self) -> Span<'src> {
        self.0
    }
}

#[cfg(test)]
mod test {
    use crate::span::SpanMethods;

    fn should_work(ip: &str) {
        match super::Ipv6Span::new(ip.as_bytes()) {
            Ok(span) => assert_eq!(
                span.span_of(ip),
                ip,
                "\n\tparsed: {:?}\n\tip    : {ip:?}",
                span.span_of(ip)
            ),
            Err(e) => assert!(
                false,
                "failed to parse\n{ip}\n{}^\n{:?} @ {}",
                &ip[0..e.1 as usize + 1],
                e.0,
                e.1
            ),
        }
    }
    fn should_fail(ip: &str) {
        match super::Ipv6Span::new(ip.as_bytes()) {
            Ok(span) => assert!(
                false,
                "should have failed to parse\n{ip}\n{}",
                span.span_of(ip),
            ),
            Err(_) => {}
        }
    }

    #[test]
    fn test_no_ipv4_mapped() {
        should_fail("[0:0:0:0:127.0.0.1]");
    }
    #[test]
    fn test_parsing_valid_ips() {
        for ip in include_str!("./valid_ipv6.tsv")
            .split("\n")
            .filter(|s| !s.is_empty())
        {
            should_work(ip)
        }
    }
}
