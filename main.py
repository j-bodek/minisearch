import uuid
from collections import defaultdict

data = [
    "droid you me look",
    "obi wan never told what happed to father",
    "father never told me that",
]


class Index:
    def __init__(self):
        self._index = defaultdict(list)
        self._documents = {}

    def _tokenize(self, data: str):
        return data.lower().split(" ")

    def _tokenize_group(self, doc):
        tokens = defaultdict(list)

        for i, token in enumerate(self._tokenize(doc)):
            tokens[token].append(i)

        return tokens.items()

    def add(self, doc: str):
        doc_id = uuid.uuid4()
        self._documents[doc_id] = doc

        for token, group in self._tokenize_group(doc):
            self._index[token].append((doc_id, (len(group), tuple(group))))

    def search(self, query: str):
        results = []

        docs = {}

        for i, token in enumerate(self._tokenize(query)):
            if token not in self._index or (i != 0 and not docs):
                return results

            new_docs = {}
            for doc_id, group in self._index[token]:
                if i != 0 and doc_id not in docs:
                    continue

                if i == 0:
                    indexes = list(group[1])
                else:
                    indexes = []

                    for index in group[1]:
                        if index - 1 in docs[doc_id]:
                            indexes.append(index)

                if indexes:
                    new_docs[doc_id] = indexes

            docs = new_docs

        for doc_id in docs.keys():
            results.append(self._documents[doc_id])

        return results


index = Index()

for d in data:
    index.add(d)

results = index.search("never told")
print(results)

# print(index._index)
