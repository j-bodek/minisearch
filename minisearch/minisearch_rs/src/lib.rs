pub mod automaton;
pub mod stemmer;
pub mod trie;
pub mod utils;

use crate::stemmer::SnowballStemmer;
use crate::trie::Trie;
use pyo3::prelude::*;

/// A Python module implemented in Rust.
#[pymodule]
fn minisearch_rs(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<SnowballStemmer>()?;
    m.add_class::<Trie>()?;
    Ok(())
}
