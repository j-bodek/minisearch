import math
import uuid
from dataclasses import dataclass
from typing import Generator
from Levenshtein import distance
from .tokenize import Tokenizer
from collections import defaultdict
from line_profiler import profile


@dataclass(slots=True)
class DocTokens:
    id: str
    num: int
    indexes: list[int]

    def __eq__(self, v):
        return self.id == v

    def __hash__(self) -> str:
        return self.id.__hash__()


class Index:
    def __init__(self):
        self._tokenizer = Tokenizer()
        self._avg_doc_len = 0
        self._index = defaultdict(set)
        self._documents = {}
        self._fuzzy_cache = {}

    def _get_tokens(self, token: str, fuzzy: int) -> Generator[str, None, None]:
        if fuzzy == 0:
            yield token
        else:
            for t in self._index.keys():
                if distance(t, token) <= fuzzy:
                    yield t

    def _flatten_docs_matches(self, tokens: list[list]):

        for t in tokens:
            flatten_token = []
            while t:
                flatten_token.append([t[0], t[1], t[2], t[3]])
                t = t[4]

            yield flatten_token

    def _bm25(
        self,
        doc_id: str,
        tokens: list[list],
        k: float = 1.5,
        b: float = 0.75,
        eps: float = 0.5,
    ) -> float:
        score = 0.0

        for flatten_tokens in self._flatten_docs_matches(tokens):

            cur_score = 0.0

            for token in flatten_tokens:
                t, tf, _, _ = token
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
                        * (
                            1
                            - b
                            + b * (self._documents[doc_id]["len"] / self._avg_doc_len)
                        )
                    )
                )

            score = max(score, cur_score)

        return score

    def add(self, doc: str) -> str:
        doc_id = str(uuid.uuid4())

        tokens_num, tokens_group = self._tokenizer.tokenize_group(doc)
        for token, group in tokens_group:
            # self._index[token].append((doc_id, [len(group), group]))
            self._index[token].add(DocTokens(id=doc_id, num=len(group), indexes=group))

        self._avg_doc_len = (self._avg_doc_len * len(self._documents) + tokens_num) / (
            len(self._documents) + 1
        )
        self._documents[doc_id] = {"len": tokens_num, "content": doc}

        return doc_id

    def cache_fuzzy(self, query, fuzzy):
        tokens = list(self._tokenizer.tokenize(query))

        for token in tokens:
            self._fuzzy_cache[token] = list(self._get_tokens(token, fuzzy))

    @profile
    def search(
        self, query: str, slop: int = 0, fuzzy: int = 0, score: bool = True
    ) -> list[dict]:

        results: list[dict] = []
        docs: dict[str, set] = {}

        tokens = list(self._tokenizer.tokenize(query))
        tokens_map = {}
        docs_set = None

        # for token in tokens:
        #     # if not (sim_tokens := list(self._get_tokens(token, fuzzy))):
        #     if not (sim_tokens := self._fuzzy_cache[token]):
        #         return results

        #     cur_docs_set = set()
        #     for t in sim_tokens:
        #         cur_docs_set = cur_docs_set.union(self._index[t])

        #     if docs_set is None:
        #         docs_set = cur_docs_set
        #     else:
        #         docs_set = docs_set.intersection(cur_docs_set)

        #     if not docs_set:
        #         return results

        #     if not docs_set:
        #         return results

        #     tokens_map[token] = sim_tokens

        for i, token in enumerate(tokens):
            if i != 0 and not docs:
                return results

            # sim_tokens = tokens_map[token]
            if not (sim_tokens := self._fuzzy_cache[token]):
                return results

            new_docs = defaultdict(set)

            for t in sim_tokens:
                for doc_tokens in self._index[t]:
                    # if doc_tokens not in docs_set:
                    #     continue

                    if i != 0 and doc_tokens.id not in docs:
                        continue

                    if i == 0:
                        new_docs[doc_tokens.id].union(doc_tokens.indexes)
                    else:
                        for index in doc_tokens.indexes:
                            if index - 1 in docs[doc_tokens.id]:
                                # for s in range(0, slop + 1):
                                #     if (
                                #         index - s - 1 in docs[doc_tokens.id]
                                #         or index + s - 1 in docs[doc_tokens.id]
                                #     ):
                                new_docs[doc_tokens.id].add(index)

            docs = new_docs

        for doc_id, values in docs.items():
            doc = self._documents[doc_id]
            result = {"content": doc["content"]}
            # if score:
            #     result["score"] = self._bm25(doc_id, values)

            results.append(result)

        # if score:
        #     return sorted(results, key=lambda x: x["score"], reverse=True)
        # else:
        return results
