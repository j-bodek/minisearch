pub mod stemmer;
pub mod utils;

use crate::stemmer::SnowballStemmer;
use pyo3::prelude::*;

/// A Python module implemented in Rust.
#[pymodule]
fn minisearch_rs(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<SnowballStemmer>()?;
    Ok(())
}
