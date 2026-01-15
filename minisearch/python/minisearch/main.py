from .rust import Search as SearchRs
from .rust import Document, Result
from typing import Self, Generator
from contextlib import contextmanager


class Index:
    def __init__(self, dir: str) -> None:
        self._search_rs = SearchRs(dir)

    @contextmanager
    def session(self) -> Generator[Self, None, None]:
        try:
            yield self
        finally:
            self.flush()

    def get(self, id: str) -> Document:
        return self._search_rs.get(id)

    def add(self, document: str) -> bool:
        return self._search_rs.add(document)

    def delete(self, id: str) -> bool:
        return self._search_rs.delete(id)

    def search(self, query: str, top_k: int = 0) -> list[Result]:
        return self._search_rs.search(query, top_k)

    def flush(self) -> None:
        return self._search_rs.flush()


class MiniSearch:

    def __init__(self):
        self._indexes: dict[str, Index] = {}

    def add(self, index: str, dir: str) -> tuple[bool, Index]:
        if index not in self._indexes:
            self._indexes[index] = Index(dir)
            return (True, self._indexes[index])

        return (False, self._indexes[index])

    def delete(self, index: str) -> None:
        if index in self._indexes:
            del self._indexes[index]

    def has_index(self, index: str) -> bool:
        return index in self._indexes

    def index(self, index: str) -> Index:
        return self._indexes[index]
