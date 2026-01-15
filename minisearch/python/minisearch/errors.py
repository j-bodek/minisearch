from .rust import (
    BincodeDecodeError,
    BincodeEncodeError,
    CompressException,
    TryFromSliceException,
    UlidDecodeError,
    UlidMonotonicError,
    UnknownLogOperation,
)


class IndexInitError(
    BincodeDecodeError,
    BincodeEncodeError,
    CompressException,
    TryFromSliceException,
    UnknownLogOperation,
):
    """Errors raised by Index.__init__"""


class IndexGetError(UlidDecodeError):
    """Errors raised by Index.get."""


class IndexAddError(
    UlidMonotonicError,
    BincodeDecodeError,
    BincodeEncodeError,
    CompressException,
    TryFromSliceException,
    UnknownLogOperation,
):
    """Errors raised by Index.add."""


class IndexDeleteError(
    UlidDecodeError,
    BincodeDecodeError,
    BincodeEncodeError,
    TryFromSliceException,
    UnknownLogOperation,
):
    """Errors raised by Index.delete."""


class IndexFlushError(BincodeEncodeError):
    """Errors raised by Index.flush."""


class IndexSessionError(BincodeEncodeError):
    """Errors raised by Index.session when flush fails on exit."""


class SearchQueryError(ValueError):
    """Errors raised by Index.search for invalid query syntax."""
