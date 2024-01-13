use crate::{
    ambiguous::host_or_path::{HostOrPathSpan, Kind as HostKind},
    err::{self, Error},
    span::{impl_span_methods_on_tuple, IntoOption, Length, Lengthy, Short},
};

fn disambiguate_err(e: Error) -> Error {
    let kind = match e.kind() {
        err::Kind::HostOrPathInvalidChar => err::Kind::HostInvalidChar,
        err::Kind::HostOrPathTooLong => err::Kind::HostTooLong,
        err::Kind::HostOrPathNoMatch => err::Kind::HostNoMatch,
        _ => e.kind(),
    };
    Error(e.index(), kind)
}

use super::ipv6::Ipv6Span;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    /// a span of characters that represents a restricted domain name, e.g. "Example.com".
    /// TODO: note the restrictions
    Domain,
    /// an IPv6 address wrapped in square brackets, e.g. "[2001:db8::1]"
    Ipv6,
    /// Missing altogether
    Empty,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) struct HostSpan<'src>(pub(crate) Length<'src>, pub(crate) Kind);
impl_span_methods_on_tuple!(HostSpan, Short);
impl<'src> TryFrom<HostOrPathSpan<'src>> for HostSpan<'_> {
    type Error = Error;
    fn try_from(ambiguous: HostOrPathSpan) -> Result<Self, Error> {
        match ambiguous.into_option() {
            None => Ok(HostSpan::none()),
            Some(_) => {
                use HostKind::*;
                match ambiguous.kind() {
                    Either | Host => Ok(Self(Length::new(ambiguous.short_len()), Kind::Domain)),
                    IpV6 => Ok(Self(Length::new(ambiguous.short_len()), Kind::Ipv6)),
                    Path => Err(Error(0, crate::err::Kind::HostInvalidChar)), // <- needs the source str to find the index of the first underscore
                }
            }
        }
    }
}

impl<'src> TryFrom<&'src str> for HostSpan<'src> {
    type Error = Error;
    fn try_from(src: &'src str) -> Result<Self, Error> {
        HostOrPathSpan::new(src, HostKind::Either)
            .map_err(disambiguate_err)?
            .try_into()
            .map_err(|e: Error| match e.kind() {
                crate::err::Kind::HostInvalidChar => Error(
                    src.find('_').unwrap().try_into().unwrap(),
                    // this error only occurs if there was an underscore in the source str,
                    // it doesn't carry the location of the offending character.
                    // Here, we find the index of the first underscore using the source str.
                    crate::err::Kind::HostInvalidChar,
                ),
                _ => e,
            })
    }
}

impl<'src> HostSpan<'src> {
    pub(crate) fn new(src: &'src str) -> Result<Self, Error> {
        // handle bracketed ipv6 addresses
        HostOrPathSpan::new(src, HostKind::Either)
            .map_err(disambiguate_err)?
            .try_into()
    }
    pub(crate) fn from_ambiguous(
        ambiguous: HostOrPathSpan<'src>,
        context: &'src str,
    ) -> Result<Self, Error> {
        match ambiguous.kind() {
            HostKind::Host | HostKind::Either => Ok(Self(ambiguous.into_length(), Kind::Domain)),
            HostKind::IpV6 => Ok(Self(ambiguous.into_length(), Kind::Ipv6)),
            HostKind::Path => Err(Error(
                ambiguous
                .span_of(context)
                .bytes().enumerate()
                .find(|(_, b)| b == &b'_')
                .map(|(i, _)| i)
                .unwrap() // safe since a Path must have at least one underscore
                .try_into()
                .unwrap(), // safe since ambiguous.span_of(context) must be short
                err::Kind::HostInvalidChar,
            )),
        }
    }
}
impl<'src> From<Ipv6Span<'src>> for HostSpan<'src> {
    fn from(ipv6: Ipv6Span<'src>) -> Self {
        Self(ipv6.short_len().into(), Kind::Ipv6)
    }
}

impl<'src> IntoOption for HostSpan<'src> {
    fn is_some(&self) -> bool {
        self.short_len() > 0
    }
    fn none() -> Self {
        Self(Length::new(0), Kind::Empty)
    }
}

pub struct HostStr<'src>(Kind, &'src str);
impl<'src> HostStr<'src> {
    pub fn src(&self) -> &'src str {
        self.1
    }
    pub fn kind(&self) -> Kind {
        self.0
    }
    #[inline(always)]
    pub fn len(&self) -> usize {
        self.src().len()
    }
    fn short_len(&self) -> Short {
        self.len().try_into().unwrap() // this is safe since the length of a HostStr is always <= U::MAX
    }
    pub(super) fn from_span_of(src: &'src str, HostSpan(span, kind): HostSpan<'src>) -> Self {
        Self(kind, span.span_of(src))
    }
    pub fn from_prefix(src: &'src str) -> Result<Self, Error> {
        let span = HostSpan::new(src)?;
        Ok(HostStr::from_span_of(src, span))
    }
    pub fn from_exact_match(src: &'src str) -> Result<Self, Error> {
        let result = HostStr::from_prefix(src)?;
        if result.len() != src.len() {
            return Err(Error(result.short_len(), crate::err::Kind::HostNoMatch));
        }
        Ok(result)
    }
}
