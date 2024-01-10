use core::marker::PhantomData;
pub type U = u8; // HACK: arbitrary limit
                 // TODO: consider increasing to u16?

pub(crate) const MAX_USIZE: usize = U::MAX as usize;

/// To avoid lugging around an entire &str (which costs 2 pointer-sizes), we can
/// use a span to represent a length of string with a lifetime tied to the original
/// string slice.
#[derive(Clone, Copy)]
pub(crate) struct OptionalSpan<'src> {
    __phantom: PhantomData<&'src str>, // tie Span to the lifetime of a string slice
    length: U,
}
impl<'src> OptionalSpan<'src> {
    // new() is needed to create a span with PhantomData tied to a specific lifetime
    pub(crate) fn new(len: U) -> Self {
        Self {
            __phantom: PhantomData,
            length: len,
        }
    }
}

impl std::ops::Add<U> for OptionalSpan<'_> {
    type Output = Self;
    fn add(self, rhs: U) -> Self {
        Self::new(self.length + rhs)
    }
}

impl From<OptionalSpan<'_>> for usize {
    fn from(span: OptionalSpan) -> Self {
        span.length as usize // U is always a small, valid usize
    }
}

/// A span that is guaranteed to be non-zero length
#[derive(Clone, Copy)]
pub(crate) struct Span<'src>(U, PhantomData<&'src str>);
impl<'src> Span<'src> {
    pub(crate) fn new(len: U) -> Self {
        debug_assert!(len > 0);
        Self(len, PhantomData)
    }
}

impl std::ops::Add<U> for Span<'_> {
    type Output = Self;
    fn add(self, rhs: U) -> Self {
        Self(self.0 + rhs, PhantomData)
    }
}

impl TryFrom<OptionalSpan<'_>> for Span<'_> {
    type Error = ();
    fn try_from(optional_span: OptionalSpan) -> Result<Self, Self::Error> {
        if optional_span.length > 0 {
            Ok(Self::new(optional_span.length))
        } else {
            Err(())
        }
    }
}

impl From<Span<'_>> for usize {
    fn from(span: Span) -> Self {
        span.0 as usize // U is always a small, valid usize
    }
}
// This conversion is safe since Span<'_> is guaranteed to be a valid OptionalSpan<'_>
impl From<Span<'_>> for OptionalSpan<'_> {
    fn from(span: Span) -> Self {
        Self::new(span.0)
    }
}

pub(crate) trait SpanMethods<'src> {
    fn short_len(&self) -> U;
    fn len(&self) -> usize {
        self.short_len() as usize
    }
    fn span_of(&self, src: &'src str) -> &'src str {
        &src[..self.len()]
    }
    fn into_span(&self) -> OptionalSpan<'src> {
        OptionalSpan::new(self.short_len())
    }
}

impl SpanMethods<'_> for OptionalSpan<'_> {
    fn short_len(&self) -> U {
        self.length
    }
}
impl SpanMethods<'_> for Span<'_> {
    fn short_len(&self) -> U {
        self.0
    }
}

/// Given a wrapper type like Wrapper<'a>(Span<'a>), re-expose the methods of Span
/// on Wrapper
macro_rules! impl_span_methods_on_tuple {
    ($id:ident) => {
        use crate::span::SpanMethods;
        impl From<$id<'_>> for usize {
            fn from(span: $id) -> Self {
                span.0.into() // U is always a small, valid usize
            }
        }
        impl<'src> SpanMethods<'src> for $id<'src> {
            #[inline(always)]
            fn short_len(&self) -> U {
                self.0.short_len()
            }
        }
    };
}
pub(crate) use impl_span_methods_on_tuple; // <- this lets us use the macro in other modules

/// Zero-length spans are inherently optional, so we can use this trait to
/// mark some kinds of spans as optional.
pub trait IntoOption
where
    Self: Sized + Copy + Clone,
{
    fn is_some(&self) -> bool;
    fn is_none(&self) -> bool {
        !self.is_some()
    }
    fn none() -> Self
    where
        Self: Sized;
    fn into_option(&self) -> Option<Self>
    where
        Self: Sized,
    {
        if self.is_some() {
            Some(*self)
        } else {
            None
        }
    }
}
impl<'src> IntoOption for OptionalSpan<'src> {
    fn none() -> Self
    where
        Self: Sized,
    {
        Self::new(0)
    }
    fn is_some(&self) -> bool {
        self.length > 0
    }
    fn into_option(&self) -> Option<OptionalSpan<'src>>
    where
        Self: Sized + Clone,
    {
        if self.is_some() {
            Some(self.clone())
        } else {
            None
        }
    }
}
