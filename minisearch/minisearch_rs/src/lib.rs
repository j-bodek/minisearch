pub mod analysis;
pub mod core;
pub mod errors;
pub mod matching;
pub mod query;
pub mod storage;
pub mod utils;

use crate::core::search::Search;
use pyo3::prelude::*;

/// A Python module implemented in Rust.
#[pymodule]
fn minisearch_rs(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<Search>()?;
    Ok(())
}
