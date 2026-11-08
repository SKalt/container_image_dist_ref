//! Python bindings for [`container_image_dist_ref`].
//!
//! The binding layer is intentionally thin: every class stores the original
//! string and re-parses it on access. Parsing is allocation-free and fast, so
//! this keeps the lifetimes in the Rust API from leaking into Python while
//! preserving the exact behavior of the core crate.

use container_image_dist_ref::{
    digest::{algorithm::Algorithm, encoded::Encoded, Compliance, Digest, Standard},
    domain::{Domain, Host, Kind as HostKind},
    err,
    name::path::Path,
    tag::Tag,
    CanonicalImgRef, ImgRef,
};
use pyo3::{create_exception, exceptions::PyValueError, prelude::*, types::PyModule};

create_exception!(container_image_dist_ref, ParseError, PyValueError);

/// Build a Python exception carrying the Rust error kind and byte index.
fn to_py_err<Size>(error: err::Error<Size>) -> PyErr
where
    Size: Copy + Into<usize>,
{
    let index: usize = error.index().into();
    let kind = format!("{:?}", error.kind());
    let py_error = ParseError::new_err(format!("{kind} at byte {index}"));
    // Exception instances have a `__dict__`, so expose the structured parts as
    // attributes while keeping `str(exc)` readable.
    Python::attach(|py| {
        let value = py_error.value(py);
        let _ = value.setattr("kind", &kind);
        let _ = value.setattr("index", index);
    });
    py_error
}

fn parse_standard(value: &str) -> PyResult<Standard> {
    match value.to_ascii_lowercase().as_str() {
        "oci" => Ok(Standard::Oci),
        "distribution" | "distribution/reference" => Ok(Standard::Distribution),
        _ => Err(PyValueError::new_err(format!(
            "unknown standard {value:?}; expected \"oci\" or \"distribution\""
        ))),
    }
}

fn compliance_str(compliance: Compliance) -> &'static str {
    match compliance {
        Compliance::Oci => "oci",
        Compliance::Distribution => "distribution",
        Compliance::Universal => "universal",
    }
}

fn compliance_from_standard(standard: Option<&str>) -> PyResult<Compliance> {
    match standard {
        None => Ok(Compliance::Universal),
        Some(standard) => Ok(match parse_standard(standard)? {
            Standard::Oci => Compliance::Oci,
            Standard::Distribution => Compliance::Distribution,
        }),
    }
}

/// A parsed image reference.
#[pyclass(name = "ImgRef", module = "container_image_dist_ref", frozen, eq, hash)]
#[derive(Clone, PartialEq, Eq, Hash)]
pub struct PyImgRef {
    src: String,
}

impl PyImgRef {
    fn parsed(&self) -> PyResult<ImgRef<'_>> {
        ImgRef::new(&self.src).map_err(to_py_err)
    }
}

#[pymethods]
impl PyImgRef {
    /// Parse an image reference. The entire string must be valid.
    #[new]
    fn new(src: &str) -> PyResult<Self> {
        ImgRef::new(src).map_err(to_py_err)?;
        Ok(Self {
            src: src.to_owned(),
        })
    }

    /// The name, including the optional domain and required path.
    #[getter]
    fn name(&self) -> PyResult<String> {
        Ok(self.parsed()?.name().to_str().to_owned())
    }

    /// The domain, if the reference includes one.
    #[getter]
    fn domain(&self) -> PyResult<Option<PyDomain>> {
        Ok(self.parsed()?.domain().map(|d| PyDomain {
            src: d.to_str().to_owned(),
        }))
    }

    /// The port, without the leading `:`.
    #[getter]
    fn port(&self) -> PyResult<Option<String>> {
        Ok(self.parsed()?.port().map(str::to_owned))
    }

    /// The path, without any leading `/` after a domain.
    #[getter]
    fn path(&self) -> PyResult<PyPath> {
        Ok(PyPath {
            src: self.parsed()?.path().to_str().to_owned(),
        })
    }

    /// The tag, without the leading `:`.
    #[getter]
    fn tag(&self) -> PyResult<Option<PyTag>> {
        Ok(self.parsed()?.tag().map(|t| PyTag { src: t.to_owned() }))
    }

    /// The digest, if present.
    #[getter]
    fn digest(&self) -> PyResult<Option<PyDigest>> {
        Ok(self.parsed()?.digest().map(|d| PyDigest {
            src: d.to_str().to_owned(),
        }))
    }

    /// Whether this reference also satisfies [`CanonicalImgRef`].
    fn is_canonical(&self) -> PyResult<bool> {
        let parsed = self.parsed()?;
        let canonical: Result<CanonicalImgRef<'_>, _> = parsed.try_into();
        Ok(canonical.is_ok())
    }

    /// Re-parse as a canonical reference, requiring a domain and digest.
    fn to_canonical(&self) -> PyResult<PyCanonicalImgRef> {
        PyCanonicalImgRef::new(&self.src)
    }

    fn __str__(&self) -> &str {
        &self.src
    }

    fn __repr__(&self) -> String {
        format!("ImgRef({:?})", self.src)
    }
}

/// A canonical image reference: a domain, path, and digest, with an optional tag.
#[pyclass(
    name = "CanonicalImgRef",
    module = "container_image_dist_ref",
    frozen,
    eq,
    hash
)]
#[derive(Clone, PartialEq, Eq, Hash)]
pub struct PyCanonicalImgRef {
    src: String,
}

impl PyCanonicalImgRef {
    fn parsed(&self) -> PyResult<CanonicalImgRef<'_>> {
        CanonicalImgRef::new(&self.src).map_err(to_py_err)
    }
}

#[pymethods]
impl PyCanonicalImgRef {
    #[new]
    fn new(src: &str) -> PyResult<Self> {
        CanonicalImgRef::new(src).map_err(to_py_err)?;
        Ok(Self {
            src: src.to_owned(),
        })
    }

    #[getter]
    fn name(&self) -> PyResult<String> {
        Ok(self.parsed()?.name().to_str().to_owned())
    }

    #[getter]
    fn domain(&self) -> PyResult<PyDomain> {
        let domain = self.parsed()?.domain();
        Ok(PyDomain {
            src: domain.to_str().to_owned(),
        })
    }

    #[getter]
    fn port(&self) -> PyResult<Option<String>> {
        Ok(self.parsed()?.domain().port().map(str::to_owned))
    }

    #[getter]
    fn path(&self) -> PyResult<PyPath> {
        Ok(PyPath {
            src: self.parsed()?.path().to_str().to_owned(),
        })
    }

    #[getter]
    fn tag(&self) -> PyResult<Option<PyTag>> {
        Ok(self.parsed()?.tag().map(|t| PyTag { src: t.to_owned() }))
    }

    #[getter]
    fn digest(&self) -> PyResult<PyDigest> {
        Ok(PyDigest {
            src: self.parsed()?.digest().to_str().to_owned(),
        })
    }

    /// Convert to a non-canonical [`ImgRef`].
    fn to_img_ref(&self) -> PyResult<PyImgRef> {
        PyImgRef::new(&self.src)
    }

    fn __str__(&self) -> &str {
        &self.src
    }

    fn __repr__(&self) -> String {
        format!("CanonicalImgRef({:?})", self.src)
    }
}

/// A parsed digest: an algorithm, an encoded value, and compliance information.
#[pyclass(name = "Digest", module = "container_image_dist_ref", frozen, eq, hash)]
#[derive(Clone, PartialEq, Eq, Hash)]
pub struct PyDigest {
    src: String,
}

impl PyDigest {
    fn parsed(&self) -> PyResult<Digest<'_>> {
        Digest::new(&self.src).map_err(to_py_err)
    }
}

#[pymethods]
impl PyDigest {
    #[new]
    fn new(src: &str) -> PyResult<Self> {
        Digest::new(src).map_err(to_py_err)?;
        Ok(Self {
            src: src.to_owned(),
        })
    }

    #[getter]
    fn algorithm(&self) -> PyResult<PyAlgorithm> {
        Ok(PyAlgorithm {
            src: self.parsed()?.algorithm().to_str().to_owned(),
        })
    }

    #[getter]
    fn encoded(&self) -> PyResult<PyEncoded> {
        Ok(PyEncoded {
            src: self.parsed()?.encoded().to_str().to_owned(),
        })
    }

    /// `"oci"`, `"distribution"`, or `"universal"`.
    #[getter]
    fn compliance(&self) -> PyResult<&'static str> {
        Ok(compliance_str(self.parsed()?.compliance()))
    }

    /// Whether the digest complies with `"oci"` or `"distribution"`.
    fn is_compliant_with(&self, standard: &str) -> PyResult<bool> {
        Ok(self
            .parsed()?
            .compliance()
            .compliant_with(parse_standard(standard)?))
    }

    fn __str__(&self) -> &str {
        &self.src
    }

    fn __repr__(&self) -> String {
        format!("Digest({:?})", self.src)
    }
}

/// A digest algorithm such as `sha256` or `sha512+mycompany`.
#[pyclass(
    name = "Algorithm",
    module = "container_image_dist_ref",
    frozen,
    eq,
    hash
)]
#[derive(Clone, PartialEq, Eq, Hash)]
pub struct PyAlgorithm {
    src: String,
}

impl PyAlgorithm {
    fn parsed(&self) -> PyResult<Algorithm<'_>> {
        Algorithm::from_exact_match(&self.src)
            .map(|(algorithm, _)| algorithm)
            .map_err(to_py_err)
    }
}

#[pymethods]
impl PyAlgorithm {
    #[new]
    fn new(src: &str) -> PyResult<Self> {
        Algorithm::from_exact_match(src).map_err(to_py_err)?;
        Ok(Self {
            src: src.to_owned(),
        })
    }

    /// The algorithm split on `+`, `.`, `_`, and `-`.
    #[getter]
    fn parts(&self) -> PyResult<Vec<String>> {
        Ok(self.parsed()?.parts().map(str::to_owned).collect())
    }

    /// `"oci"`, `"distribution"`, or `"universal"`.
    #[getter]
    fn compliance(&self) -> PyResult<&'static str> {
        Ok(compliance_str(self.parsed()?.compliance()))
    }

    fn is_compliant_with(&self, standard: &str) -> PyResult<bool> {
        Ok(self
            .parsed()?
            .compliance()
            .compliant_with(parse_standard(standard)?))
    }

    fn __str__(&self) -> &str {
        &self.src
    }

    fn __repr__(&self) -> String {
        format!("Algorithm({:?})", self.src)
    }
}

/// The encoded value of a digest.
#[pyclass(
    name = "Encoded",
    module = "container_image_dist_ref",
    frozen,
    eq,
    hash
)]
#[derive(Clone, PartialEq, Eq, Hash)]
pub struct PyEncoded {
    src: String,
}

#[pymethods]
impl PyEncoded {
    /// Parse an encoded digest value.
    ///
    /// `standard` may be `"oci"`, `"distribution"`, or `None` (the default,
    /// which accepts everything either standard allows).
    #[new]
    #[pyo3(signature = (src, standard=None))]
    fn new(src: &str, standard: Option<&str>) -> PyResult<Self> {
        let compliance = compliance_from_standard(standard)?;
        Encoded::new(src, compliance).map_err(to_py_err)?;
        Ok(Self {
            src: src.to_owned(),
        })
    }

    fn __str__(&self) -> &str {
        &self.src
    }

    fn __repr__(&self) -> String {
        format!("Encoded({:?})", self.src)
    }
}

/// A domain: a host and an optional port.
#[pyclass(name = "Domain", module = "container_image_dist_ref", frozen, eq, hash)]
#[derive(Clone, PartialEq, Eq, Hash)]
pub struct PyDomain {
    src: String,
}

impl PyDomain {
    fn parsed(&self) -> PyResult<Domain<'_>> {
        Domain::from_exact_match(&self.src).map_err(to_py_err)
    }
}

#[pymethods]
impl PyDomain {
    #[new]
    fn new(src: &str) -> PyResult<Self> {
        Domain::from_exact_match(src).map_err(to_py_err)?;
        Ok(Self {
            src: src.to_owned(),
        })
    }

    #[getter]
    fn host(&self) -> PyResult<PyHost> {
        Ok(PyHost {
            src: self.parsed()?.host().to_str().to_owned(),
        })
    }

    #[getter]
    fn port(&self) -> PyResult<Option<String>> {
        Ok(self.parsed()?.port().map(str::to_owned))
    }

    fn __str__(&self) -> &str {
        &self.src
    }

    fn __repr__(&self) -> String {
        format!("Domain({:?})", self.src)
    }
}

/// A host name or a bracketed IPv6 address.
#[pyclass(name = "Host", module = "container_image_dist_ref", frozen, eq, hash)]
#[derive(Clone, PartialEq, Eq, Hash)]
pub struct PyHost {
    src: String,
}

impl PyHost {
    fn parsed(&self) -> PyResult<Host<'_>> {
        Host::from_exact_match(&self.src).map_err(to_py_err)
    }
}

#[pymethods]
impl PyHost {
    #[new]
    fn new(src: &str) -> PyResult<Self> {
        Host::from_exact_match(src).map_err(to_py_err)?;
        Ok(Self {
            src: src.to_owned(),
        })
    }

    /// `"name"` for a domain name (including IPv4) or `"ipv6"`.
    #[getter]
    fn kind(&self) -> PyResult<&'static str> {
        Ok(match self.parsed()?.kind() {
            HostKind::Name => "name",
            HostKind::Ipv6 => "ipv6",
        })
    }

    fn __str__(&self) -> &str {
        &self.src
    }

    fn __repr__(&self) -> String {
        format!("Host({:?})", self.src)
    }
}

/// A repository path.
#[pyclass(name = "Path", module = "container_image_dist_ref", frozen, eq, hash)]
#[derive(Clone, PartialEq, Eq, Hash)]
pub struct PyPath {
    src: String,
}

#[pymethods]
impl PyPath {
    /// Parse a path. Parsing stops at `:`, `@`, or the end of the string.
    #[new]
    fn new(src: &str) -> PyResult<Self> {
        let path = Path::new(src).map_err(to_py_err)?;
        Ok(Self {
            src: path.to_str().to_owned(),
        })
    }

    /// The `/`-delimited components of the path.
    #[getter]
    fn parts(&self) -> Vec<String> {
        self.src.split('/').map(str::to_owned).collect()
    }

    fn __str__(&self) -> &str {
        &self.src
    }

    fn __repr__(&self) -> String {
        format!("Path({:?})", self.src)
    }
}

/// A tag.
#[pyclass(name = "Tag", module = "container_image_dist_ref", frozen, eq, hash)]
#[derive(Clone, PartialEq, Eq, Hash)]
pub struct PyTag {
    src: String,
}

#[pymethods]
impl PyTag {
    /// Parse a tag. Parsing stops at `@` or the end of the string.
    #[new]
    fn new(src: &str) -> PyResult<Self> {
        let tag = Tag::new(src).map_err(to_py_err)?;
        Ok(Self {
            src: tag.to_str().to_owned(),
        })
    }

    fn __str__(&self) -> &str {
        &self.src
    }

    fn __repr__(&self) -> String {
        format!("Tag({:?})", self.src)
    }
}

/// Parse an image reference. The entire string must be valid.
#[pyfunction]
fn parse(reference: &str) -> PyResult<PyImgRef> {
    PyImgRef::new(reference)
}

/// Parse a canonical image reference (required domain and digest).
#[pyfunction]
fn parse_canonical(reference: &str) -> PyResult<PyCanonicalImgRef> {
    PyCanonicalImgRef::new(reference)
}

/// Parse a digest such as `"sha256:aaaa..."`. The entire string must be valid.
#[pyfunction]
fn parse_digest(src: &str) -> PyResult<PyDigest> {
    PyDigest::new(src)
}

/// Parse a digest algorithm such as `"sha256"`. The entire string must be valid.
#[pyfunction]
fn parse_algorithm(src: &str) -> PyResult<PyAlgorithm> {
    PyAlgorithm::new(src)
}

/// Parse an encoded digest value. `standard` may be `"oci"`, `"distribution"`,
/// or `None`.
#[pyfunction]
#[pyo3(signature = (src, standard=None))]
fn parse_encoded(src: &str, standard: Option<&str>) -> PyResult<PyEncoded> {
    PyEncoded::new(src, standard)
}

/// Parse a domain such as `"localhost:5000"`. The entire string must be valid.
#[pyfunction]
fn parse_domain(src: &str) -> PyResult<PyDomain> {
    PyDomain::new(src)
}

/// Parse a host name or bracketed IPv6 address. The entire string must be valid.
#[pyfunction]
fn parse_host(src: &str) -> PyResult<PyHost> {
    PyHost::new(src)
}

/// Parse a repository path. Parsing stops at `:`, `@`, or the end of the input.
#[pyfunction]
fn parse_path(src: &str) -> PyResult<PyPath> {
    PyPath::new(src)
}

/// Parse a tag. Parsing stops at `@` or the end of the input.
#[pyfunction]
fn parse_tag(src: &str) -> PyResult<PyTag> {
    PyTag::new(src)
}

#[pymodule]
fn _native(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_class::<PyImgRef>()?;
    module.add_class::<PyCanonicalImgRef>()?;
    module.add_class::<PyDigest>()?;
    module.add_class::<PyAlgorithm>()?;
    module.add_class::<PyEncoded>()?;
    module.add_class::<PyDomain>()?;
    module.add_class::<PyHost>()?;
    module.add_class::<PyPath>()?;
    module.add_class::<PyTag>()?;

    module.add("ParseError", module.py().get_type::<ParseError>())?;

    module.add_function(wrap_pyfunction!(parse, module)?)?;
    module.add_function(wrap_pyfunction!(parse_canonical, module)?)?;
    module.add_function(wrap_pyfunction!(parse_digest, module)?)?;
    module.add_function(wrap_pyfunction!(parse_algorithm, module)?)?;
    module.add_function(wrap_pyfunction!(parse_encoded, module)?)?;
    module.add_function(wrap_pyfunction!(parse_domain, module)?)?;
    module.add_function(wrap_pyfunction!(parse_host, module)?)?;
    module.add_function(wrap_pyfunction!(parse_path, module)?)?;
    module.add_function(wrap_pyfunction!(parse_tag, module)?)?;

    module.add("__version__", env!("CARGO_PKG_VERSION"))?;
    Ok(())
}
