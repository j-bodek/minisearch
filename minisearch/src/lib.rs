pub mod analysis;
pub mod config;
pub mod core;
pub mod errors;
pub mod matching;
pub mod query;
pub mod storage;
pub mod utils;

use pyo3::prelude::*;

#[pymodule]
mod rust {
    #[pymodule_export]
    use crate::core::search::PySearchResult;
    #[pymodule_export]
    use crate::core::search::Search;
    #[pymodule_export]
    use crate::storage::documents::Document;

    // errors
    #[pymodule_export]
    use crate::errors::BincodeDecodeError;
    #[pymodule_export]
    use crate::errors::BincodeEncodeError;
    #[pymodule_export]
    use crate::errors::CompressException;
    #[pymodule_export]
    use crate::errors::TryFromSliceException;
    #[pymodule_export]
    use crate::errors::UlidDecodeError;
    #[pymodule_export]
    use crate::errors::UlidMonotonicError;
    #[pymodule_export]
    use crate::errors::UnknownLogOperation;
}
