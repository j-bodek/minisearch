import heapq
import bisect
import math
import uuid
from .tokenize import Tokenizer
from collections import defaultdict
from minisearch_rs import Trie


class Index:
    def __init__(self):
        self._tokenizer = Tokenizer()
        self._avg_doc_len = 0
        self._index = defaultdict(list)
        self._documents = {}
        self._fuzzy_trie = Trie()
        for d in range(4):
            self._fuzzy_trie.init_automaton(d)

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
            bisect.insort(
                self._index[token], (doc_id, [len(group), group]), key=lambda x: x[0]
            )
            self._fuzzy_trie.add(token)

        self._avg_doc_len = (self._avg_doc_len * len(self._documents) + tokens_num) / (
            len(self._documents) + 1
        )
        self._documents[doc_id] = {"len": tokens_num, "content": doc}

        return doc_id

    def search(
        self, query: str, slop: int = 0, fuzzy: int = 0, score: bool = True
    ) -> list[dict]:

        results: list[dict] = []
        docs: dict[str, list] = {}

        for i, token in enumerate(self._tokenizer.tokenize(query)):
            tokens = self._fuzzy_trie.search(fuzzy, token)
            if not tokens or (i != 0 and not docs):
                return results

            new_docs = defaultdict(list)
            for t in tokens:
                for doc_id, group in self._index[t]:
                    if i != 0 and doc_id not in docs:
                        continue

                    if i == 0:
                        indexes = [[t, 0, group[0], group[1], None]]
                    else:
                        indexes = []

                        for prev_token in docs[doc_id]:
                            for index in group[1]:
                                for s in range(0, (slop - prev_token[1]) + 1):
                                    if (
                                        index - s - 1 in prev_token[3]
                                        or index + s - 1 in prev_token[3]
                                    ):
                                        indexes.append(
                                            [
                                                t,
                                                prev_token[1] + s,
                                                group[0],
                                                group[1],
                                                prev_token,
                                            ]
                                        )

                    if indexes:
                        new_docs[doc_id].extend(indexes)

            docs = new_docs

        for doc_id, values in docs.items():
            doc = self._documents[doc_id]
            result = {"content": doc["content"]}
            if score:
                result["score"] = self._bm25(doc_id, values)

            results.append(result)

        if score:
            return sorted(results, key=lambda x: x["score"], reverse=True)
        else:
            return results

    def match(self, tokens, doc_indexes, min_slop):
        indexes = []
        slop, token_indexes = 0, [0 for _ in range(len(tokens))]

        # init indexes
        for i, token in enumerate(tokens):
            doc_idx = doc_indexes[i]
            token_indexes[i] = self._index[token][doc_idx][1][1][0]
            heapq.heappush(indexes, (self._index[token][doc_idx][1][1][0], 0, i))

            if i > 0:
                slop += abs(token_indexes[i - 1] - token_indexes[i])

        while True:
            # check if min slop is matched
            if slop <= min_slop:
                yield token_indexes

            token_idx, idx, token_id = heapq.heappop(indexes)
            token, elem_idx = (
                tokens[token_id],
                doc_indexes[token_id],
            )
            if idx + 1 > len(self._index[token][elem_idx][1][1]) - 1:
                break

            token_idx = self._index[token][elem_idx][1][1][idx + 1]

            # update slop
            if token_id > 0:
                # update next slop
                slop -= abs(token_indexes[token_id - 1] - token_indexes[token_id])
                slop += abs(token_indexes[token_id - 1] - token_idx)

            if token_id < len(token_indexes) - 1:
                # update previous slop
                slop -= abs(token_indexes[token_id] - token_indexes[token_id + 1])
                slop += abs(token_idx - token_indexes[token_id + 1])

            token_indexes[token_id] = token_idx
            heapq.heappush(
                indexes,
                (self._index[token][elem_idx][1][1][idx + 1], idx + 1, token_id),
            )

    def _search(
        self, query: str, slop: int = 0, fuzzy: int = 0, score: bool = True
    ) -> list[dict]:
        results = []
        target_doc = None
        docs, indexes, same = [], [], True
        tokens = list(self._tokenizer.tokenize(query))

        for t in tokens:
            if t not in self._index:
                return results

            doc_id = self._index[t][0][0]
            if docs and docs[-1] != doc_id:
                same = False

            docs.append(doc_id)
            indexes.append(0)

        target_doc = max(docs)

        # find intersection on docs
        while True:
            if same:
                for token_indexes in self.match(tokens, indexes, slop):
                    results.append((doc_id, token_indexes, self._documents[doc_id]))

                same = True
                for i, token in enumerate(tokens):
                    if indexes[i] + 1 > len(self._index[token]) - 1:
                        break

                    indexes[i] += 1
                    doc_id = self._index[token][indexes[i]][0]
                    docs[i] = doc_id
                    target_doc = max(target_doc, doc_id)
                    if i != 0 and doc_id != docs[i - 1]:
                        same = False
                else:
                    continue

                break
            else:
                same = True
                for i, token in enumerate(tokens):
                    new_idx = bisect.bisect_left(
                        self._index[token], target_doc, key=lambda x: x[0]
                    )

                    if new_idx > len(self._index[token]) - 1:
                        break

                    indexes[i] = new_idx
                    doc_id = self._index[token][new_idx][0]
                    docs[i] = doc_id
                    target_doc = max(target_doc, doc_id)
                    if i != 0 and doc_id != docs[i - 1]:
                        same = False
                else:
                    continue

                break

        return results
