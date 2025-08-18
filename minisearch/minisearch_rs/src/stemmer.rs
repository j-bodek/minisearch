use pyo3::prelude::*;

#[pyclass(name = "SnowballStemmer")]
pub struct SnowballStemmer {
    r1: u32,
    r2: u32,
}

#[pymethods]
impl SnowballStemmer {
    #[new]
    fn new() -> Self {
        SnowballStemmer { r1: 0, r2: 0 }
    }

    fn stem(&self, v: String) -> String {
        v
    }
}
