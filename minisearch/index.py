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

    def _bm25_new(
        self,
        doc_id: str,
        token_groups: list[list],
        k: float = 1.5,
        b: float = 0.75,
        eps: float = 0.5,
    ) -> float:
        score = 0.0

        for tokens in token_groups:

            cur_score = 0.0

            for token in tokens:
                _, t, tf = token
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

    def _match(self, docs, min_slop):
        result = []
        tfs = {}
        cur_indexes = []
        slop, indexes_window = 0, [0 for _ in range(len(docs))]
        token_groups = defaultdict(lambda: {"heap": []})

        def get_next(group_id):
            if len(token_groups[group_id]["heap"]) == 0:
                return None

            token_idx, group_id, token, doc_idx, idx = heapq.heappop(
                token_groups[group_id]["heap"]
            )
            if idx + 1 <= len(self._index[token][doc_idx][1][1]) - 1:
                new_token_idx = self._index[token][doc_idx][1][1][idx + 1]
                heapq.heappush(
                    token_groups[group_id]["heap"],
                    (new_token_idx, group_id, token, doc_idx, idx + 1),
                )

            return (token_idx, group_id, token, doc_idx, idx)

        for i, group in enumerate(docs):
            for item in group:
                _, token, doc_idx = item
                if token not in tfs:
                    tfs[token] = len(self._index[token][doc_idx][1][1])

                idx = self._index[token][doc_idx][1][1][0]
                heapq.heappush(token_groups[i]["heap"], (idx, i, token, doc_idx, 0))

            indexes_window[i] = (
                token_groups[i]["heap"][0][0],
                token_groups[i]["heap"][0][2],
                tfs[token_groups[i]["heap"][0][2]],
            )
            heapq.heappush(cur_indexes, get_next(i))

        while cur_indexes:
            # check slop
            slop = 0

            for i in range(len(indexes_window) - 1):
                slop += abs(indexes_window[i][0] - (indexes_window[i + 1][0] - 1))

            if slop <= min_slop:
                result.append(indexes_window.copy())

            token_idx, group_id, token, doc_idx, idx = heapq.heappop(cur_indexes)
            _next = get_next(group_id)

            if _next is None:
                continue

            indexes_window[group_id] = (_next[0], _next[2], tfs[_next[2]])
            heapq.heappush(cur_indexes, _next)

        return result

    def _next_doc_index(self, pointers):
        doc_ids = []
        while pointers["heap"] and (
            len(doc_ids) == 0 or doc_ids[0][0] == pointers["heap"][0][0]
        ):
            doc_id, token = heapq.heappop(pointers["heap"])

            idx = pointers["tokens_doc_idx"][token]

            if idx + 1 <= len(self._index[token]) - 1:
                heapq.heappush(
                    pointers["heap"], (self._index[token][idx + 1][0], token)
                )
                pointers["tokens_doc_idx"][token] += 1
            else:
                del pointers["tokens_doc_idx"][token]

            doc_ids.append((doc_id, token, idx))

        return doc_ids

    def _geq_doc_index(self, pointers, target_doc):
        while pointers["heap"] and pointers["heap"][0][0] < target_doc:
            _, token = heapq.heappop(pointers["heap"])
            new_idx = bisect.bisect_left(
                self._index[token], target_doc, key=lambda x: x[0]
            )
            if new_idx > len(self._index[token]) - 1:
                del pointers["tokens_doc_idx"][token]
            else:
                pointers["tokens_doc_idx"][token] = new_idx
                heapq.heappush(
                    pointers["heap"], (self._index[token][new_idx][0], token)
                )

        return self._next_doc_index(pointers)

    def _search(
        self, query: str, slop: int = 0, fuzzy: int = 0, score: bool = True
    ) -> list[dict]:
        results = defaultdict(list)
        target_doc = None
        docs, indexes, same = [], [], True
        pointers = defaultdict(lambda: {"heap": [], "tokens_doc_idx": {}})
        tokens = list(self._tokenizer.tokenize(query))

        for token in tokens:
            if token not in self._index:
                return []

            for t in self._fuzzy_trie.search(fuzzy, token):
                heapq.heappush(pointers[token]["heap"], (self._index[t][0][0], t))
                pointers[token]["tokens_doc_idx"][t] = 0

            docs_ids = self._next_doc_index(pointers[token])
            if docs and docs[-1][0][0] != docs_ids[0][0]:
                same = False

            docs.append(docs_ids)
            indexes.append(0)

        target_doc = max(docs, key=lambda x: x[0][0])[0][0]

        # find intersection on docs
        while True:
            if same:
                doc_id = docs[0][0][0]
                for token_indexes in self._match(docs, slop):
                    results[doc_id].append(token_indexes)

                same = True
                for i, token in enumerate(tokens):
                    docs_ids = self._next_doc_index(pointers[token])
                    if len(docs_ids) == 0:
                        break

                    docs[i] = docs_ids
                    target_doc = max(target_doc, docs_ids[0][0])

                    if i != 0 and docs[i][0][0] != docs[i - 1][0][0]:
                        same = False
                else:
                    continue

                break
            else:
                same, cur_target_doc = True, target_doc
                for i, token in enumerate(tokens):
                    if cur_target_doc != docs[i][0][0]:
                        docs_ids = self._geq_doc_index(pointers[token], target_doc)

                        if len(docs_ids) == 0:
                            break

                        docs[i] = docs_ids
                        target_doc = max(target_doc, docs_ids[0][0])

                    if i != 0 and docs[i][0][0] != docs[i - 1][0][0]:
                        same = False
                else:
                    continue

                break

        _results = []
        for doc_id, token_groups in results.items():
            doc = self._documents[doc_id]
            result = {"content": doc["content"]}
            if score:
                result["score"] = self._bm25_new(doc_id, token_groups)

            _results.append(result)

        return sorted(_results, key=lambda x: x["score"], reverse=True)
