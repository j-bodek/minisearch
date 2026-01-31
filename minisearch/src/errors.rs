use std::{io, time::SystemTimeError};

use bincode::error::{DecodeError, EncodeError};
use pyo3::{create_exception, exceptions::PySystemError};
use thiserror::Error;

create_exception!(crate, TryFromSliceException, pyo3::exceptions::PyException);
create_exception!(crate, UnknownLogOperation, pyo3::exceptions::PyException);
create_exception!(crate, BincodeEncodeError, pyo3::exceptions::PyException);
create_exception!(crate, BincodeDecodeError, pyo3::exceptions::PyException);
create_exception!(crate, UlidMonotonicError, pyo3::exceptions::PyException);
create_exception!(crate, UlidDecodeError, pyo3::exceptions::PyException);
create_exception!(crate, CompressException, pyo3::exceptions::PyException);
create_exception!(
    crate,
    TomlDeserializeException,
    pyo3::exceptions::PyException
);

#[derive(Error, Debug)]
pub enum BincodePersistenceError {
    #[error(transparent)]
    Io(#[from] io::Error),
    #[error(transparent)]
    Time(#[from] SystemTimeError),
    #[error(transparent)]
    BincodeEncodeError(#[from] EncodeError),
    #[error(transparent)]
    BincodeDecodeError(#[from] DecodeError),
}

impl From<BincodePersistenceError> for pyo3::PyErr {
    fn from(err: BincodePersistenceError) -> Self {
        match err {
            BincodePersistenceError::Io(err) => err.into(),
            BincodePersistenceError::Time(err) => PySystemError::new_err(err.to_string()),
            BincodePersistenceError::BincodeEncodeError(err) => {
                BincodeEncodeError::new_err(err.to_string())
            }
            BincodePersistenceError::BincodeDecodeError(err) => {
                BincodeDecodeError::new_err(err.to_string())
            }
        }
    }
}
