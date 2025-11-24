from .index import Index
from minisearch_rs import Index as IndexRs


class MiniSearch:

    def __init__(self):
        self._indexes: dict[str, Index] = {}

    def add(self, index: str) -> None:
        if index not in self._indexes:
            self._indexes[index] = Index()

    def delete(self, index: str) -> None:
        if index in self._indexes:
            del self._indexes[index]

    def has_index(self, index: str) -> bool:
        return index in self._indexes

    def index(self, index: str) -> Index:
        return self._indexes[index]


class MiniSearchRs:

    def __init__(self):
        self._indexes: dict[str, Index] = {}

    def add(self, index: str) -> None:
        if index not in self._indexes:
            self._indexes[index] = IndexRs()

    def delete(self, index: str) -> None:
        if index in self._indexes:
            del self._indexes[index]

    def has_index(self, index: str) -> bool:
        return index in self._indexes

    def index(self, index: str) -> Index:
        return self._indexes[index]
