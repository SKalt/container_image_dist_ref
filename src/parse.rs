pub(crate) fn as_result(parse: Parse) -> Result<Parse, Parse> {
    if parse.len > 0 && parse.compliance != Compliance::Uncompliant {
        Ok(parse)
    } else {
        Err(parse)
    }
}

pub struct Parse {
    pub len: u8,
    pub compliance: Compliance,
}

pub enum Standard {
    /// Though distribution/reference isn't officially a standard or specification
    /// as the de-facto reference implementation for references, we'll treat it as
    /// a standard.
    Distribution,
    Oci,
}
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Compliance {
    /// Not compliant with distribution/reference: at least one algorithm component
    /// starts with a number.
    Oci,
    /// Not compliant with OCI image spec: at least one letter is uppercase.
    Distribution,
    /// Compliant with both distribution/reference and OCI image spec.
    Universal,
    /// Not compliant with either distribution/reference or OCI image spec.
    /// This implies at least one algorithm component starts with a number, but
    /// the algorithm string includes uppercase letters.
    Uncompliant,
}

impl Compliance {
    pub fn compliant_with(self, standard: Standard) -> bool {
        match (self, Standard) {
            (Compliance::Universal, _) => true,
            (Compliance::Oci, Standard::Oci) => true,
            (Compliance::Distribution, Standard::Distribution) => true,
            _ => false,
        }
    }
}
