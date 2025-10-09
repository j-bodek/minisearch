import heapq
import bisect
import math

# import uuid
from ulid import monotonic as ulid
from .tokenize import Tokenizer
from collections import defaultdict
from minisearch_rs import Trie


class TokensIterator:
    def __init__(self):
        self.heap = []
        self.generators = []
        self.gen_meta = []
        self.last_gen_id = None

    def init_generator(self, token, tfs, gen):
        try:
            heapq.heappush(self.heap, (next(gen), len(self.generators)))
            self.generators.append(gen)
            self.gen_meta.append((token, tfs))
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

        return None, None


class WindowAndIterator:
    def __init__(self, min_slop):
        self.min_slop = min_slop
        self.slop = 0
        self.window = []
        self.heap = []
        self.iterators = []

    def init_iterator(self, iterator):
        val = iterator.peak()
        if val is not None:
            heapq.heappush(self.heap, (val, len(self.iterators)))
            self.iterators.append(iterator)
            if self.window:
                self.slop += abs(self.window[-1] - (val - 1))

            self.window.append(val)

    def full_window(self):
        window = []
        for iter_id, idx in enumerate(self.window):
            token, tfs = self.iterators[iter_id].last_meta()
            window.append((idx, token, tfs))
        return window

    def next(self):
        if not self.heap:
            return None, None

        top = self.heap[0][0]
        while self.heap and top == self.heap[0][0]:
            _, iter_id = heapq.heappop(self.heap)
            if (val := self.iterators[iter_id].next()) is not None:
                heapq.heappush(self.heap, (val, iter_id))

        return self.heap[0] if self.heap else (None, None)

    def join(self):
        while True:
            if self.slop <= self.min_slop:
                yield self.full_window()

            val, iter_id = self.next()
            if val is None:
                break

            if iter_id > 0:
                self.slop -= abs(self.window[iter_id - 1] - (self.window[iter_id] - 1))
                self.slop += abs(self.window[iter_id - 1] - (val - 1))

            if iter_id < len(self.window) - 1:
                self.slop -= abs(self.window[iter_id] - (self.window[iter_id + 1] - 1))
                self.slop += abs(val - (self.window[iter_id + 1] - 1))

            self.window[iter_id] = val


class Index:
    def __init__(self):
        self._tokenizer = Tokenizer()
        self._avg_doc_len = 0
        self._index = defaultdict(list)
        self._blocks = defaultdict(list)
        self._documents = {}
        self._fuzzy_trie = Trie()
        self._BLOCK_SIZE = 256
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

    def _bm25(self, doc_id: str, token_groups: list[list]) -> float:
        score = 0.0

        for tokens in token_groups:

            cur_score = 0.0

            for token in tokens:
                _, t, tf = token
                cur_score += self._tf_norm(doc_id, t, tf)

            score = max(score, cur_score)

        return score

    def add(self, doc: str) -> str:
        # doc_id = str(uuid.uuid4())
        doc_id = str(ulid.new())

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

    def _slow_match(self, docs, min_slop):
        tfs = {}

        for group in docs:
            for item in group:
                _, token, doc_idx = item
                if token not in tfs:
                    tfs[token] = len(self._index[token][doc_idx][1][1])

        def join(idx):
            if idx > len(docs) - 1:
                return []

            elements = list(join(idx=idx + 1))
            for item in docs[idx]:
                _, token, doc_idx = item
                for e in self._index[token][doc_idx][1][1]:
                    if idx < len(docs) - 1:
                        for slop, el in elements:
                            if el[0][0] > e and el[0][0] <= e + (min_slop - slop) + 1:
                                # if (
                                #     el[0][0] >= e - (min_slop - slop) - 1
                                #     and el[0][0] <= e + (min_slop - slop) + 1
                                #     and (e, token) not in el
                                # ):
                                yield slop + abs(e - (el[0][0] - 1)), [(e, token)] + el
                    else:
                        yield 0, [(e, token)]

        for slop, window in join(idx=0):

            if slop <= min_slop:
                yield [(i, token, tfs[token]) for (i, token) in window]

    def _match_mis_queue(self, docs, min_slop):

        window_iterator = WindowAndIterator(min_slop=min_slop)

        for group in docs:
            tokens_iterator = TokensIterator()

            for item in group:
                _, token, doc_idx = item

                tokens_iterator.init_generator(
                    token,
                    self._index[token][doc_idx][1][0],
                    (x for x in self._index[token][doc_idx][1][1]),
                )

            window_iterator.init_iterator(tokens_iterator)

        yield from window_iterator.join()

    def _match_mis_gready(self, docs, min_slop):

        token_iterators = []

        for group in docs:
            tokens_iterator = TokensIterator()

            for item in group:
                _, token, doc_idx = item

                tokens_iterator.init_generator(
                    token,
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
                    token, tfs = token_iterators[iter_id].last_meta()
                    w.append((idx, token, tfs))
                yield w

            if (val := token_iterators[0].next()) is None:
                break

            i = 1
            window[0] = val

    def _match_mis(self, docs, min_slop):

        token_iterators = []
        tfs = {}

        for group in docs:
            tokens_iterator = TokensIterator()

            for item in group:
                _, token, doc_idx = item

                tfs[token] = len(self._index[token][doc_idx][1][1])
                tokens_iterator.init_generator(
                    token,
                    self._index[token][doc_idx][1][0],
                    (x for x in self._index[token][doc_idx][1][1]),
                )

            token_iterators.append(tokens_iterator)

        def combine(idx=0):
            if idx > len(docs) - 1:
                return []

            val, token = token_iterators[idx].peak()
            el_idx, elements = 0, list(combine(idx=idx + 1))
            while val is not None:
                if idx < len(docs) - 1 and elements:
                    if elements[-1][1][0][0] < val - min_slop - 1:
                        break

                    for i in range(max(0, el_idx - min_slop - 2), len(elements)):
                        el_idx = i
                        slop, el = elements[i]
                        if (
                            el[0][0] >= val - (min_slop - slop) - 1
                            and el[0][0] <= val + (min_slop - slop) + 1
                            and (val, token) not in el
                        ):
                            yield slop + abs(val - (el[0][0] - 1)), [(val, token)] + el
                        elif el[0][0] > val + min_slop + 1:
                            break

                elif idx == len(docs) - 1:
                    yield 0, [(val, token)]

                val, token = token_iterators[idx].next()

        for slop, window in combine():
            if slop <= min_slop:
                yield [(i, token, tfs[token]) for (i, token) in window]

    def _match(self, docs, min_slop):
        results = []
        indexes, tfs = defaultdict(list), {}

        for group_id, group in enumerate(docs):
            heap, token_indexes = [], {}
            for item in group:
                _, token, doc_idx = item
                tfs[token] = len(self._index[token][doc_idx][1][1])
                heapq.heappush(
                    heap, (self._index[token][doc_idx][1][1][0], token, doc_idx)
                )
                token_indexes[token] = 0

            while token_indexes:
                idx, token, doc_idx = heapq.heappop(heap)
                token_indexes[token] += 1
                if len(self._index[token][doc_idx][1][1]) - 1 < token_indexes[token]:
                    del token_indexes[token]
                else:
                    heapq.heappush(
                        heap,
                        (
                            self._index[token][doc_idx][1][1][token_indexes[token]],
                            token,
                            doc_idx,
                        ),
                    )

                indexes[group_id].append((idx, token))

        window = [(None, None) for i in range(len(indexes))]
        window_indexes = [(-1, -1) for i in range(len(indexes))]

        def get_first_closest(start, target, indexes, cur_index, slop):
            _next = None
            for i in range(max(start - slop, 0), len(indexes)):
                if (
                    indexes[i][0] >= target - slop
                    and indexes[i] not in window[:cur_index]
                ):
                    diff = abs(target - (indexes[i][0] - 1))
                    if diff <= slop:
                        _next = i
                        break
                    elif indexes[i][0] > target + slop:
                        break

            if _next is not None:
                return indexes[_next], _next
            else:
                return None, -1

        def get_next(index, indexes, cur_index):
            while index + 1 <= len(indexes) - 1:
                if indexes[index + 1] not in window[:cur_index]:
                    return indexes[index + 1], index + 1

                index += 1

            return None, -1

        cur_index, back, end_reached = 0, False, None
        while True:
            if (cur_index == end_reached and back) or cur_index < 0:
                break

            if cur_index == 0 or back:
                next_item, next_i = get_next(
                    window_indexes[cur_index][0], indexes[cur_index], cur_index
                )
            else:
                next_item, next_i = get_first_closest(
                    window_indexes[cur_index][0],
                    window[cur_index - 1][0],
                    indexes[cur_index],
                    cur_index,
                    min_slop - window_indexes[cur_index - 1][1],
                )

            if next_item is None and (cur_index == 0 or back):
                end_reached = (
                    min(end_reached, cur_index)
                    if end_reached is not None
                    else cur_index
                )

            if next_item is None:
                cur_index -= 1
                back = True
            else:
                cur_slop = window_indexes[cur_index - 1][1] if cur_index > 0 else 0
                diff = (
                    abs(window[cur_index - 1][0] - (next_item[0] - 1))
                    if cur_index > 0
                    else 0
                )

                if cur_slop + diff > min_slop:
                    cur_index -= 1
                    back = True
                elif cur_index == len(window) - 1:
                    window[cur_index] = next_item
                    window_indexes[cur_index] = (next_i, cur_slop + diff)
                    results.append([(i, token, tfs[token]) for (i, token) in window])
                    cur_index -= 1
                    back = True
                else:
                    window[cur_index] = next_item
                    window_indexes[cur_index] = (next_i, cur_slop + diff)
                    cur_index += 1
                    back = False

        return results

    def _next_doc_index(self, pointers):
        max_score, doc_ids = 0, []
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

            max_score = max(max_score, self._index[token][idx][2])
            doc_ids.append((doc_id, token, idx))

        return max_score, doc_ids

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

    def top_search(
        self,
        query: str,
        slop: int = 0,
        fuzzy: int = 0,
        top_k: int = None,
        score: bool = True,
    ) -> list[dict]:
        results = []
        target_doc = None
        docs, indexes, same = [], [], True
        max_scores = []
        pointers = defaultdict(lambda: {"heap": [], "tokens_doc_idx": {}})
        tokens = list(self._tokenizer.tokenize(query))

        for token_id, token in enumerate(tokens):

            for t in self._fuzzy_trie.search(fuzzy, token):
                heapq.heappush(pointers[token_id]["heap"], (self._index[t][0][0], t))
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
                    for token_indexes in self._match_mis_gready(docs, slop):
                        matches.append(token_indexes)

                    if matches:
                        # calculate score
                        doc = self._documents[doc_id]
                        score = self._bm25(doc_id, matches)
                        if not top_k or len(results) < top_k:
                            heapq.heappush(results, ((score, doc["content"])))
                        elif score > results[0][0]:
                            heapq.heappop(results)
                            heapq.heappush(results, ((score, doc["content"])))

                same = True
                for i, token in enumerate(tokens):
                    max_score, docs_ids = self._next_doc_index(pointers[i])
                    if len(docs_ids) == 0:
                        break

                    docs[i] = docs_ids
                    max_scores[i] = max_score
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
                        max_score, docs_ids = self._geq_doc_index(
                            pointers[i], target_doc
                        )

                        if len(docs_ids) == 0:
                            break

                        docs[i] = docs_ids
                        max_scores[i] = max_score
                        target_doc = max(target_doc, docs_ids[0][0])

                    if i != 0 and docs[i][0][0] != docs[i - 1][0][0]:
                        same = False
                else:
                    continue

                break

        return [
            {"score": s, "content": c}
            for s, c in sorted(results, key=lambda x: x[0], reverse=True)
        ]

    def search(
        self,
        query: str,
        slop: int = 0,
        fuzzy: int = 0,
        top_k: int = None,
        score: bool = True,
    ) -> list[dict]:
        results = defaultdict(list)
        target_doc = None
        docs, indexes, same = [], [], True
        pointers = defaultdict(lambda: {"heap": [], "tokens_doc_idx": {}})
        tokens = list(self._tokenizer.tokenize(query))

        for token_id, token in enumerate(tokens):

            for t in self._fuzzy_trie.search(fuzzy, token):
                heapq.heappush(pointers[token_id]["heap"], (self._index[t][0][0], t))
                pointers[token_id]["tokens_doc_idx"][t] = 0

            if len(pointers[token_id]["heap"]) == 0:
                return []

            _, docs_ids = self._next_doc_index(pointers[token_id])
            if docs and docs[-1][0][0] != docs_ids[0][0]:
                same = False

            docs.append(docs_ids)
            indexes.append(0)

        target_doc = max(docs, key=lambda x: x[0][0])[0][0]

        # find intersection on docs
        while True:
            if same:
                doc_id = docs[0][0][0]
                for token_indexes in self._match_mis_gready(docs, slop):
                    results[doc_id].append(token_indexes)

                same = True
                for i, token in enumerate(tokens):
                    _, docs_ids = self._next_doc_index(pointers[i])
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
                        _, docs_ids = self._geq_doc_index(pointers[i], target_doc)

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
                result["score"] = self._bm25(doc_id, token_groups)

            _results.append(result)

        if score:
            _results = sorted(_results, key=lambda x: x["score"], reverse=True)
            if top_k:
                return _results[:top_k]

            return _results
        else:
            return _results

    """
    HELPER
    """

    def _bm25_slow(
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

                cur_score += idf * (
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

    def _slow_match_2(self, doc, tokens, fuzzy, min_slop):
        result = []
        tfs = {}
        indexes = [[] for _ in range(len(tokens))]

        for i, t in enumerate(tokens):
            for token in self._fuzzy_trie.search(fuzzy, t):
                doc_idx = [
                    i
                    for i in range(len(self._index[token]))
                    if self._index[token][i][0] == doc
                ]

                if not doc_idx:
                    continue

                doc_idx = doc_idx[0]
                if token not in tfs:
                    tfs[token] = len(self._index[token][doc_idx][1][1])

                indexes[i].extend(
                    [(idx, token) for idx in self._index[token][doc_idx][1][1]]
                )

        def combine(indexes, idx=0):
            if idx > len(indexes) - 1:
                return []

            elements = list(combine(indexes, idx=idx + 1))
            for e in indexes[idx]:
                if idx < len(indexes) - 1:
                    for el in elements:
                        if el[0][0] > e[0] and el[0][0] <= e[0] + min_slop + 1:
                            # if (
                            #     e not in el
                            #     and el[0][0] >= e[0] - min_slop - 1
                            #     and el[0][0] <= e[0] + min_slop + 2
                            # ):
                            yield [e] + el
                else:
                    yield [e]

        for window in combine(indexes):
            slop = 0
            for i in range(len(window) - 1):
                slop += abs(window[i][0] - (window[i + 1][0] - 1))

            if slop <= min_slop:
                result.append([(i, token, tfs[token]) for (i, token) in window])
        return result

    def _slow_search(
        self,
        query: str,
        slop: int = 0,
        fuzzy: int = 0,
        top_k: int = None,
        score: bool = True,
    ) -> list[dict]:
        results = defaultdict(list)
        docs = set()
        tokens = list(self._tokenizer.tokenize(query))

        for i, token in enumerate(tokens):
            if i > 0 and not docs:
                return []

            cur_docs = set()
            fuzzy_tokens = list(self._fuzzy_trie.search(fuzzy, token))
            if not fuzzy_tokens:
                return []

            for t in fuzzy_tokens:
                for d in self._index[t]:
                    cur_docs.add(d[0])

            if i == 0:
                docs = cur_docs
            else:
                docs = docs.intersection(cur_docs)

        for d in docs:
            for token_indexes in self._slow_match_2(d, tokens, fuzzy, slop):
                results[d].append(token_indexes)

        _results = []
        for doc_id, token_groups in results.items():
            doc = self._documents[doc_id]
            result = {"content": doc["content"]}
            if score:
                result["score"] = self._bm25_slow(doc_id, token_groups)

            _results.append(result)

        if score:
            _results = sorted(_results, key=lambda x: x["score"], reverse=True)
            if top_k:
                return _results[:top_k]

            return _results
        else:
            return _results
