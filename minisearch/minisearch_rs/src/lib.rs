pub mod automaton;
pub mod documents;
pub mod index;
pub mod intersect;
pub mod mis;
pub mod parser;
pub mod scoring;
pub mod search;
pub mod stemmer;
pub mod tokenizer;
pub mod trie;
pub mod utils;

use crate::search::Search;
use pyo3::prelude::*;

/// A Python module implemented in Rust.
#[pymodule]
fn minisearch_rs(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<Search>()?;
    Ok(())
}
