//! non-backtracking parsers for ambiguous sections of the reference grammar.
//! Each parser retains information that helps disambiguate the result and throw
//! meaningful errors.
pub(crate) mod domain_or_tagged_ref;
pub(crate) mod host_or_path;
pub(crate) mod port_or_tag;
