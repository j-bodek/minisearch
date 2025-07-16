import re
import uuid
import Stemmer
from stop import stop_words
from collections import defaultdict


data = [
    "These     are not the droids you are looking for.",
    "Obi-Wan never told you what happened to your father.",
    "No. I am your father.",
]


class Index:
    def __init__(self):
        self._stemmer = Stemmer.Stemmer("english")
        self._index = defaultdict(list)
        self._documents = {}

    def _tokenize(self, data: str):
        for token in re.sub("[^A-Za-z0-9\s]+", "", data).lower().split():
            if token in stop_words:
                continue

            yield self._stemmer.stemWord(token)

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


for r in index.search("I am your father"):
    print(r)
