pub mod automaton;
pub mod index;
pub mod parser;
pub mod scoring;
pub mod stemmer;
pub mod tokenizer;
pub mod trie;
pub mod utils;

use crate::index::Index;
use pyo3::prelude::*;

/// A Python module implemented in Rust.
#[pymodule]
fn minisearch_rs(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<Index>()?;
    Ok(())
}
