//! types + traits for working with lengths of strings
use core::marker::PhantomData;
pub type Short = u8;
pub type Long = u16;

/// To avoid lugging around an entire &str (which costs 2 pointer-sizes), we can
/// use a span to represent a length of string with a lifetime tied to the original
/// string slice.
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct Length<'src, Size = Short>(
    Size,
    PhantomData<&'src str>, // tie Span to the lifetime of a string slice
);
impl<'src, Size> Length<'src, Size> {
    // new() is needed to create a span with PhantomData tied to a specific lifetime
    pub(crate) fn new(len: Size) -> Self {
        Self(len, PhantomData)
    }
}
impl<Size> From<Size> for Length<'_, Size> {
    fn from(len: Size) -> Self {
        Self::new(len)
    }
}
pub type ShortLength<'src> = Length<'src, Short>;
pub type LongLength<'src> = Length<'src, u16>;

impl<Size: Into<usize>> From<Length<'_, Size>> for usize {
    fn from(span: Length<Size>) -> usize {
        span.0.into()
    }
}

pub(crate) trait Lengthy<'src, Size>
where
    Self: Sized,
    usize: From<Size>,
{
    fn short_len(&self) -> Size;
    fn len(&self) -> usize {
        self.short_len().into()
    }
    fn span_of(&self, src: &'src str) -> &'src str {
        &src[..self.len()]
    }
    fn into_length(self) -> Length<'src, Size> {
        self.short_len().into()
    }
}

impl<Size> Lengthy<'_, Size> for Length<'_, Size>
where
    usize: From<Size>,
    Size: Copy,
{
    fn short_len(&self) -> Size {
        self.0
    }
}

/// Given a wrapper type like Wrapper<'a>(Span<'a>), re-expose the methods of Span
/// on Wrapper
macro_rules! impl_span_methods_on_tuple {
    ($id:ident, $size:ident) => {
        impl From<$id<'_>> for usize {
            fn from(span: $id) -> Self {
                span.0.into() // U is always a small, valid usize
            }
        }
        impl<'src> crate::span::Lengthy<'src, crate::span::$size> for $id<'src> {
            #[inline(always)]
            fn short_len(&self) -> crate::span::$size {
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
    fn into_option(self) -> Option<Self> {
        if self.is_some() {
            Some(self)
        } else {
            None
        }
    }
}

impl<'src> IntoOption for Length<'src> {
    fn none() -> Self {
        Self::new(0)
    }
    fn is_some(&self) -> bool {
        self.0 > 0
    }
    fn into_option(self) -> Option<Length<'src>>
    where
        Self: Sized + Clone,
    {
        if self.is_some() {
            Some(self)
        } else {
            None
        }
    }
}
