from .rust import Search as SearchRs
from .rust import Document, Result
from typing import Generator
from contextlib import contextmanager


class Index:
    def __init__(self, dir: str) -> None:
        """
        Create or load an index stored in "dir"

        Raises:
            IndexInitError: load/create index state failed
        """
        self._search_rs = SearchRs(dir)

    @contextmanager
    def session(self) -> Generator[None, None, None]:
        """
        Context manager that always flushes on exit

        Raises:
            IndexSessionError: flush on exit failed
        """
        try:
            yield None
        finally:
            self.flush()

    def get(self, id: str) -> Document:
        """
        Fetch a document by ULID string

        Raises:
            IndexGetError: invalid ULID
        """
        return self._search_rs.get(id)

    def add(self, document: str) -> bool:
        """
        Add a document and return its ULID string

        Raises:
            IndexAddError: add operation failed
        """
        return self._search_rs.add(document)

    def delete(self, id: str) -> bool:
        """
        Mark a document deleted

        Raises:
            IndexDeleteError: delete operation failed
        """
        return self._search_rs.delete(id)

    def search(self, query: str, top_k: int = 0) -> list[Result]:
        """
        Search the index and return scored results

        Raises:
            SearchQueryError: invalid query syntax
        """
        return self._search_rs.search(query, top_k)

    def flush(self) -> None:
        """
        Persist all buffered changes

        Raises:
            IndexFlushError: flush failed
        """
        return self._search_rs.flush()


class MiniSearch:

    def __init__(self):
        """Create an in-memory registry of indexes"""
        self._indexes: dict[str, Index] = {}

    def add(self, index: str, dir: str) -> tuple[bool, Index]:
        """
        Get or create an index handle

        Raises:
            IndexInitError: load/create index state failed
        """
        if index not in self._indexes:
            self._indexes[index] = Index(dir)
            return (True, self._indexes[index])

        return (False, self._indexes[index])

    def delete(self, index: str) -> None:
        """Remove an index handle from the registry"""
        if index in self._indexes:
            del self._indexes[index]

    def has_index(self, index: str) -> bool:
        """Return True if the index handle exists"""
        return index in self._indexes

    def index(self, index: str) -> Index:
        """Fetch an existing index handle"""
        return self._indexes[index]
