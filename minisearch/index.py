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

    # def search(
    #     self, query: str, slop: int = 0, fuzzy: int = 0, score: bool = True
    # ) -> list[dict]:

    #     results: list[dict] = []
    #     docs: dict[str, list] = {}

    #     for i, token in enumerate(self._tokenizer.tokenize(query)):
    #         tokens = self._fuzzy_trie.search(fuzzy, token)
    #         if not tokens or (i != 0 and not docs):
    #             return results

    #         new_docs = defaultdict(list)
    #         for t in tokens:
    #             for doc_id, group in self._index[t]:
    #                 if i != 0 and doc_id not in docs:
    #                     continue

    #                 if i == 0:
    #                     indexes = [[t, 0, group[0], group[1], None]]
    #                 else:
    #                     indexes = []

    #                     for prev_token in docs[doc_id]:
    #                         for index in group[1]:
    #                             for s in range(0, (slop - prev_token[1]) + 1):
    #                                 if (
    #                                     index - s - 1 in prev_token[3]
    #                                     or index + s - 1 in prev_token[3]
    #                                 ):
    #                                     indexes.append(
    #                                         [
    #                                             t,
    #                                             prev_token[1] + s,
    #                                             group[0],
    #                                             group[1],
    #                                             prev_token,
    #                                         ]
    #                                     )

    #                 if indexes:
    #                     new_docs[doc_id].extend(indexes)

    #         docs = new_docs

    #     for doc_id, values in docs.items():
    #         doc = self._documents[doc_id]
    #         result = {"content": doc["content"]}
    #         if score:
    #             result["score"] = self._bm25(doc_id, values)

    #         results.append(result)

    #     if score:
    #         return sorted(results, key=lambda x: x["score"], reverse=True)
    #     else:
    #         return results

    def match(self, pointers, min_slop):
        indexes = []
        slop, token_indexes = 0, [0 for _ in pointers.keys()]

        # init indexes
        for i, v in pointers.items():
            token, idx = v["token"], v["elem_idx"]
            token_indexes[i] = self._index[token][idx][1][1][0]
            heapq.heappush(indexes, (self._index[token][idx][1][1][0], 0, i))

            if i > 0:
                slop += abs(token_indexes[i - 1] - token_indexes[i])

        while True:
            # check if min slop is matched
            if slop <= min_slop:
                yield token_indexes

            token_idx, idx, token_id = heapq.heappop(indexes)
            token, elem_idx = (
                pointers[token_id]["token"],
                pointers[token_id]["elem_idx"],
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

    def search(
        self, query: str, slop: int = 0, fuzzy: int = 0, score: bool = True
    ) -> list[dict]:

        results = []
        ids, pointers, ids_map = [], {}, defaultdict(int)
        tokens = self._tokenizer.tokenize(query)

        # init pointers
        for i, t in enumerate(tokens):
            if t not in self._index:
                # token don't exists
                return results

            pointers[i] = {
                "token": t,
                "skip_len": math.floor(
                    math.sqrt(len(self._index[t]))
                ),  # to dynamically compute next skip pointer
                "elem_idx": 0,
            }

            heapq.heappush(ids, (self._index[t][0][0], i))
            ids_map[self._index[t][0][0]] += 1

        while True:
            if len(ids_map) == 1:
                doc_id = ids[0][0]
                for indexes in self.match(pointers, slop):
                    results.append((doc_id, indexes, self._documents[doc_id]))

            doc_id, pointer_id = heapq.heappop(ids)

            if doc_id in ids_map:
                ids_map[doc_id] -= 1
                if ids_map[doc_id] == 0:
                    del ids_map[doc_id]

            token, skip_len, idx = (
                pointers[pointer_id]["token"],
                pointers[pointer_id]["skip_len"],
                pointers[pointer_id]["elem_idx"],
            )

            # use skip pointer
            next_idx = (idx // skip_len + 1) * skip_len
            while (
                next_idx < len(self._index[token]) - 1
                and self._index[token][next_idx][0] < ids[0][0]
            ):
                idx = next_idx
                next_idx = (idx // skip_len + 1) * skip_len

            if idx + 1 > len(self._index[token]) - 1:
                break

            heapq.heappush(ids, (self._index[token][idx + 1][0], pointer_id))
            pointers[pointer_id]["elem_idx"] += 1
            ids_map[self._index[token][idx + 1][0]] += 1

        print(results)
        return results
