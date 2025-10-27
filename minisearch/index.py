import heapq
import bisect
import math

from ulid import monotonic as ulid
from .tokenize import Tokenizer
from .parser import QueryParser
from collections import defaultdict
from minisearch_rs import Trie
from line_profiler import profile


class TokensIterator:
    def __init__(self):
        self.heap = []
        self.generators = []
        self.gen_meta = []
        self.last_gen_id = None

    def init_generator(self, token, distance, tfs, gen):
        try:
            heapq.heappush(self.heap, (next(gen), len(self.generators)))
            self.generators.append(gen)
            self.gen_meta.append((token, distance, tfs))
        except StopIteration:
            pass

    def closest(self, target):
        while self.heap and self.heap[0][0] <= target:
            val = None
            _, gen_id = heapq.heappop(self.heap)
            while True:
                try:
                    if (val := next(self.generators[gen_id])) > target:
                        break
                except StopIteration:
                    break

            if val is not None:
                heapq.heappush(self.heap, (val, gen_id))

        if self.heap:
            self.last_gen_id = self.heap[0][1]
            return self.heap[0][0]

        return None

    def next(self):
        if self.heap:
            _, gen_id = heapq.heappop(self.heap)
            try:
                heapq.heappush(self.heap, (next(self.generators[gen_id]), gen_id))
            except StopIteration:
                pass

        if self.heap:
            self.last_gen_id = self.heap[0][1]
            return self.heap[0][0]

        return None

    def peak(self):
        if self.heap:
            self.last_gen_id = self.heap[0][1]
            return self.heap[0][0]

        return None

    def last_meta(self):
        if self.last_gen_id is not None:
            return self.gen_meta[self.last_gen_id]

        return None, None, None


class Index:
    def __init__(self):
        self._tokenizer = Tokenizer()
        self._parser = QueryParser()
        self._avg_doc_len = 0
        self._index = defaultdict(list)
        self._documents = {}
        self._fuzzy_trie = Trie()
        for d in range(4):
            self._fuzzy_trie.init_automaton(d)

    def _tf_norm(
        self,
        doc_id,
        token,
        tf,
        k: float = 1.5,
        b: float = 0.75,
        eps: float = 0.5,
    ):
        idf = math.log(
            (
                (len(self._documents) - len(self._index[token]) + eps)
                / (len(self._index[token]) + eps)
            )
            + 1
        )

        return idf * (
            (tf * (k + 1))
            / (
                tf
                + k * (1 - b + b * (self._documents[doc_id]["len"] / self._avg_doc_len))
            )
        )

    def _bm25(
        self, doc_id: str, token_groups: list[list], fp_const: float = 0.8
    ) -> float:
        score = 0.0

        for slop, tokens in token_groups:

            cur_score = 0.0

            for token in tokens:
                _, t, tf, distance = token
                # tf norm and fuzziness penalty
                cur_score += self._tf_norm(doc_id, t, tf) * math.pow(fp_const, distance)

            # slop penalty
            cur_score /= slop + 1
            score = max(score, cur_score)

        return score

    def add(self, doc: str) -> str:
        doc_id = str(ulid.new())  # TODO: research ulid

        tokens_num, tokens_group = self._tokenizer.tokenize_group(doc)
        self._avg_doc_len = (self._avg_doc_len * len(self._documents) + tokens_num) / (
            len(self._documents) + 1
        )
        self._documents[doc_id] = {"len": tokens_num, "content": doc}

        for token, group in tokens_group:
            self._index[token].append([doc_id, [len(group), group], 0])
            self._fuzzy_trie.add(token)
            self._index[token][-1][2] = self._tf_norm(doc_id, token, len(group))

        return doc_id

    def match(self, docs, min_slop):

        token_iterators = []

        for group in docs:
            tokens_iterator = TokensIterator()

            for item in group:
                _, token, doc_idx, distance = item

                tokens_iterator.init_generator(
                    token,
                    distance,
                    self._index[token][doc_idx][1][0],
                    (x for x in self._index[token][doc_idx][1][1]),
                )

            token_iterators.append(tokens_iterator)

        window = [token_iterators[i].peak() for i in range(len(token_iterators))]
        slops = [0 for _ in range(len(token_iterators))]

        i = 1
        while True:
            end = False
            while i <= len(window) - 1:
                if (val := token_iterators[i].closest(window[i - 1])) is not None:
                    window[i] = val
                else:
                    end = True
                    break

                slop = slops[i - 1] + abs(window[i - 1] - (window[i] - 1))
                if slop > min_slop:
                    break

                slops[i] = slop
                i += 1

            if end:
                break

            if i > len(window) - 1:
                w = []
                for iter_id, idx in enumerate(window):
                    token, distance, tfs = token_iterators[iter_id].last_meta()
                    w.append((idx, token, tfs, distance))

                yield slops[-1], w

            if (val := token_iterators[0].next()) is None:
                break

            i = 1
            window[0] = val

    def _next_doc_index(self, pointers):
        max_score, doc_ids = 0, []
        while pointers["heap"] and (
            len(doc_ids) == 0 or doc_ids[0][0] == pointers["heap"][0][0]
        ):
            doc_id, token, d = heapq.heappop(pointers["heap"])
            idx = pointers["tokens_doc_idx"][token]

            if idx + 1 <= len(self._index[token]) - 1:
                heapq.heappush(
                    pointers["heap"], (self._index[token][idx + 1][0], token, d)
                )
                pointers["tokens_doc_idx"][token] += 1
            else:
                del pointers["tokens_doc_idx"][token]

            max_score = max(max_score, self._index[token][idx][2])
            doc_ids.append((doc_id, token, idx, d))

        return max_score, doc_ids

    def _geq_doc_index(self, pointers, target_doc):
        while pointers["heap"] and pointers["heap"][0][0] < target_doc:
            _, token, d = heapq.heappop(pointers["heap"])
            new_idx = bisect.bisect_left(
                self._index[token], target_doc, key=lambda x: x[0]
            )
            if new_idx > len(self._index[token]) - 1:
                del pointers["tokens_doc_idx"][token]
            else:
                pointers["tokens_doc_idx"][token] = new_idx
                heapq.heappush(
                    pointers["heap"], (self._index[token][new_idx][0], token, d)
                )

        return self._next_doc_index(pointers)

    @profile
    def search(
        self,
        query: str,
        top_k: int = None,
        score: bool = True,
    ) -> list[dict]:
        results = []
        target_doc = None
        docs, indexes, same = [], [], True
        max_scores = []
        pointers = defaultdict(lambda: {"heap": [], "tokens_doc_idx": {}})

        query, slop = self._parser.parse_slop(query.lower())
        tokens = list(
            self._tokenizer.tokenize_query(self._parser.parse_fuzziness(query))
        )

        for token_id, (token, fuzzy) in enumerate(tokens):
            fuzzy = max(fuzzy, 0)
            for d, t in self._fuzzy_trie.search(fuzzy, token):
                if t != token and (len(t) <= fuzzy or len(token) <= fuzzy):
                    continue

                heapq.heappush(pointers[token_id]["heap"], (self._index[t][0][0], t, d))
                pointers[token_id]["tokens_doc_idx"][t] = 0

            if len(pointers[token_id]["heap"]) == 0:
                return []

            max_score, docs_ids = self._next_doc_index(pointers[token_id])
            if docs and docs[-1][0][0] != docs_ids[0][0]:
                same = False

            max_scores.append(max_score)
            docs.append(docs_ids)
            indexes.append(0)

        target_doc = max(docs, key=lambda x: x[0][0])[0][0]

        # find intersection on docs
        while True:
            if same:
                doc_id = docs[0][0][0]
                if (
                    not top_k  # either 0 or None
                    or len(results) < top_k
                    or sum(max_scores) > results[0][0]
                ):
                    matches = []
                    for s, token_indexes in self.match(docs, slop):
                        matches.append((s, token_indexes))

                    if matches:
                        # calculate score
                        doc = self._documents[doc_id]
                        score = self._bm25(doc_id, matches)
                        if not top_k or len(results) < top_k:
                            heapq.heappush(results, (score, doc["content"]))
                        elif score > results[0][0]:
                            heapq.heappop(results)
                            heapq.heappush(results, (score, doc["content"]))

                same, end = True, False
                for i, token in enumerate(tokens):
                    max_score, docs_ids = self._next_doc_index(pointers[i])
                    if len(docs_ids) == 0:
                        end = True
                        break

                    docs[i] = docs_ids
                    max_scores[i] = max_score
                    target_doc = max(target_doc, docs_ids[0][0])

                    if i != 0 and docs[i][0][0] != docs[i - 1][0][0]:
                        same = False

                if end:
                    break
            else:
                same, end, cur_target_doc = True, False, target_doc
                for i, token in enumerate(tokens):
                    if cur_target_doc != docs[i][0][0]:
                        max_score, docs_ids = self._geq_doc_index(
                            pointers[i], target_doc
                        )

                        if len(docs_ids) == 0:
                            end = True
                            break

                        docs[i] = docs_ids
                        max_scores[i] = max_score
                        target_doc = max(target_doc, docs_ids[0][0])

                    if i != 0 and docs[i][0][0] != docs[i - 1][0][0]:
                        same = False

                if end:
                    break

        return [
            {"score": s, "content": c}
            for s, c in sorted(results, key=lambda x: x[0], reverse=True)
        ]
