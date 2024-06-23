//! types + traits for working with lengths of strings
use core::marker::PhantomData;
use core::num::{NonZeroU16, NonZeroU8};

// need this trait since I can't implement From<NonZeroU8> for usize
pub trait OptionallyZero
where
    Self: Sized + Clone,
{
    type Possible: From<Self> + Into<usize> + Into<u16> + Sized;
    fn new(val: Self::Possible) -> Option<Self>;
    #[inline]
    fn upcast(&self) -> Self::Possible {
        self.clone().into()
    }
    #[inline]
    fn widen(&self) -> NonZeroU16 {
        unsafe { NonZeroU16::new_unchecked(self.upcast().into()) }
    }
    #[inline]
    fn as_usize(&self) -> usize {
        self.upcast().into()
    }
}

impl OptionallyZero for NonZeroU8 {
    type Possible = u8;

    #[inline]
    fn new(val: Self::Possible) -> Option<NonZeroU8> {
        Self::new(val)
    }
}
impl OptionallyZero for NonZeroU16 {
    type Possible = u16;

    #[inline]
    fn new(val: Self::Possible) -> Option<NonZeroU16> {
        Self::new(val)
    }
}

/// Safely mark a numeric literal as nonzero
macro_rules! nonzero {
    (u8, $n:expr) => {
        unsafe {
            debug_assert!($n != 0u8);
            NonZeroU8::new_unchecked($n)
        }
    };
    (u16, $n:expr) => {
        unsafe {
            debug_assert!($n != 0);
            NonZeroU16::new_unchecked($n)
        }
    };
}

pub(crate) use nonzero;

// pub(crate) const fn safe_add

/// To avoid lugging around an entire &str (which costs 2 pointer-sizes), we can
/// use a span to represent a length of string with a lifetime tied to the original
/// string slice.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Length<'src, NonZeroLength: Clone + Copy>(
    NonZeroLength,
    PhantomData<&'src str>,
    // ensure this length doesn't outlive the string slice it's derived from
);

impl<'src, NonZero, Original> Length<'src, NonZero>
where
    NonZero: OptionallyZero<Possible = Original> + Clone + Copy,
{
    /// new() is needed to create a span with `PhantomData` tied to a specific lifetime.
    /// Returns None iff the input length is zero.
    pub(crate) fn new(len: Original) -> Option<Self> {
        NonZero::new(len).map(|len| Self::from_nonzero(len))
    }
    #[inline]
    pub(crate) const fn from_nonzero(len: NonZero) -> Self {
        Self(len, PhantomData)
    }
}

pub type ShortLength<'src> = Length<'src, NonZeroU8>;
pub type LongLength<'src> = Length<'src, NonZeroU16>;

// I think this impl can be eliminated
impl<NonZero, Original> From<Length<'_, NonZero>> for usize
where
    NonZero: OptionallyZero<Possible = Original> + Clone + Copy,
    Original: Into<usize>,
{
    #[inline]
    fn from(span: Length<NonZero>) -> usize {
        span.0.as_usize()
    }
}

#[allow(clippy::len_without_is_empty)]
pub(crate) trait Lengthy<'src, OriginalSize, NonZeroSize>
where
    Self: Sized,
    OriginalSize: Into<usize>,
    NonZeroSize: OptionallyZero<Possible = OriginalSize> + Clone + Copy,
{
    fn short_len(&self) -> NonZeroSize;

    #[inline]
    fn into_length(self) -> Option<Length<'src, NonZeroSize>> {
        Length::new(self.short_len().upcast())
    }
    #[inline]
    fn len(&self) -> usize {
        self.short_len().as_usize()
    }
    #[inline]
    fn span_of(&self, src: &'src str) -> &'src str {
        &src[..self.len()] // FIXME: use .get() to avoid panics
    }
}

impl<Size, Original> Lengthy<'_, Original, Size> for Length<'_, Size>
where
    Size: OptionallyZero<Possible = Original> + Clone + Copy,
    Original: Into<usize> + From<Original>,
{
    #[inline]
    fn short_len(&self) -> Size {
        self.0
    }
    #[inline]
    fn len(&self) -> usize {
        self.0.as_usize()
    }
}
impl From<Length<'_, NonZeroU16>> for u16 {
    #[inline]
    fn from(span: Length<'_, NonZeroU16>) -> u16 {
        u16::from(span.0)
    }
}
impl From<Length<'_, NonZeroU8>> for u16 {
    #[inline]
    fn from(span: Length<'_, NonZeroU8>) -> u16 {
        u8::from(span.0).into()
    }
}

/// Given a wrapper type like Wrapper<'a>(Span<'a>), re-expose the methods of Span
/// on Wrapper
macro_rules! impl_span_methods_on_tuple {
    ($id:ident, $orig:ident, $size:ident) => {
        impl From<$id<'_>> for usize {
            #[inline]
            fn from(span: $id) -> Self {
                // U is always a small, valid usize
                span.0.len()
            }
        }
        impl<'src> crate::span::Lengthy<'src, $orig, $size> for $id<'src> {
            #[inline]
            fn short_len(&self) -> $size {
                self.0.short_len()
            }
            #[inline]
            fn len(&self) -> usize {
                self.0.len()
            }
        }
    };
}
pub(crate) use impl_span_methods_on_tuple; // <- this lets us use the macro in other modules
