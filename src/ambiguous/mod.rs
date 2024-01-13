use crate::span::{IntoOption, Short};

pub(crate) mod domain_or_tagged_ref;
pub(crate) mod host_or_path;
pub(crate) mod port_or_tag;

/// The index of the first byte that determines the kind of a host/path or port/tag.
#[derive(Clone, Copy)]
pub struct Discriminant(Short);
impl Discriminant {
    /// since all hosts/paths and ports/tags must be under 255 ascii bytes long,
    /// the pattern 255 is a niche that can be used to indicate that the
    /// deciding byte is not present.
    const NONE: Short = Short::MAX;
}
impl IntoOption for Discriminant {
    fn is_some(&self) -> bool {
        self.0 != Self::NONE
    }
    fn none() -> Self {
        Self(Self::NONE)
    }
}
impl From<Option<Discriminant>> for Discriminant {
    fn from(d: Option<Discriminant>) -> Self {
        d.unwrap_or_else(Self::none)
    }
}
impl core::ops::BitOr<Discriminant> for Option<Discriminant> {
    type Output = Self;
    fn bitor(self, rhs: Discriminant) -> Self::Output {
        self.or(rhs.into())
    }
}

impl core::ops::BitOrAssign<Discriminant> for Option<Discriminant> {
    fn bitor_assign(&mut self, rhs: Discriminant) {
        *self = match self {
            Some(_) => *self,
            None => rhs.into(),
        }
    }
}
