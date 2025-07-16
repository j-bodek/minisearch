import math
import re
import uuid
import Stemmer
from stop import stop_words
from Levenshtein import distance
from collections import defaultdict
from rank_bm25 import BM25Okapi


data = [
    "These     are not the droids you are looking for.",
    "Obi-Wan never told you what happened to your father.",
    "No. I am your father.",
    "ant bat cat dog elephant fish giraffe",
]


class Index:
    def __init__(self):
        self._stemmer = Stemmer.Stemmer("english")
        self._avg_doc_len = 0
        self._index = defaultdict(list)
        self._documents = {}

    def _tokenize(self, data: str):
        for token in re.sub("[^A-Za-z0-9\s]+", "", data).lower().split():
            if token in stop_words:
                continue

            yield self._stemmer.stemWord(token)

    def _tokenize_group(self, doc):
        tokens = defaultdict(list)

        i = 0
        for i, token in enumerate(self._tokenize(doc)):
            tokens[token].append(i)

        return i + 1, tokens.items()

    def _get_tokens(self, token: str, fuzzyness: int):
        if fuzzyness == 0:
            yield token
        else:
            for t in self._index.keys():
                if distance(t, token) <= fuzzyness:
                    yield t

    def _bm25(
        self,
        doc_id: str,
        tokens: list[list],
        k: float = 1.5,
        b: float = 0.75,
        eps: float = 0.5,
    ):
        score = 0

        for token in tokens:
            t, tf, _ = token
            idf = math.log(
                (
                    (len(self._documents) - len(self._index[t]) + eps)
                    / (len(self._index[t]) + eps)
                )
                + 1
            )

            score += idf * (
                (tf * (k + 1))
                / (
                    tf
                    + k
                    * (1 - b + b * (self._documents[doc_id]["len"] / self._avg_doc_len))
                )
            )

        return score

    def add(self, doc: str):
        doc_id = str(uuid.uuid4())

        tokens_num, tokens_group = self._tokenize_group(doc)
        for token, group in tokens_group:
            self._index[token].append((doc_id, [len(group), group]))

        self._avg_doc_len = (self._avg_doc_len * len(self._documents) + tokens_num) / (
            len(self._documents) + 1
        )
        self._documents[doc_id] = {"len": tokens_num, "content": doc}

        return doc_id

    def search(self, query: str, slop: int = 0, fuzzyness: int = 0):
        results = []

        docs = {}

        for i, token in enumerate(self._tokenize(query)):
            tokens = list(self._get_tokens(token, fuzzyness))
            if not tokens or (i != 0 and not docs):
                return results

            new_docs = {}
            for t in tokens:
                for doc_id, group in self._index[t]:
                    if i != 0 and doc_id not in docs:
                        continue

                    if i == 0:
                        indexes = [t, group[0], group[1]]
                    else:
                        indexes = [t, group[0], []]

                        for index in group[1]:
                            for s in range(-(slop - 1), slop + 2):
                                if index - s in docs[doc_id][-1][2]:
                                    indexes[2].append(index)

                    if indexes[2]:
                        new_docs[doc_id] = [*docs.get(doc_id, []), indexes]

                docs = new_docs

        for doc_id, values in docs.items():
            doc = self._documents[doc_id]
            results.append(
                {"score": self._bm25(doc_id, values), "content": doc["content"]}
            )

        return sorted(results, key=lambda x: x["score"], reverse=True)


index = Index()

for d in data:
    index.add(d)


for r in index.search("i am your father", slop=3):
    print(r)
