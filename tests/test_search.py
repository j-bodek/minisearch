import time
import json
import pytest
from minisearch import MiniSearch


@pytest.fixture
def data():
    with open("tests/assets/articles_1k.json", "r+") as f:
        data = json.load(f)

    return data.values()


@pytest.fixture
def queries():
    queries = []
    with open("tests/assets/queries.txt", "r+") as f:
        for q in f.readlines():
            queries.append(q.strip())

    return queries


@pytest.fixture
def results():
    with open("tests/assets/results_1k_top_k.json", "r+") as f:
        data = json.load(f)

    return data


def rust_query(query, fuzzy, slop):
    return '"' + " ".join([f"{t}~{fuzzy}" for t in query.lower().split()]) + f'"~{slop}'


def test_performance(data, queries, results):

    search = MiniSearch()
    s = time.time()
    _, index = search.add("wikipedia", "data")
    print(f"Loading took: {time.time() - s}")

    s = time.time()
    with index.session():
        for d in data:
            index.add(d)

    print(f"Inserting took: {time.time() - s}")

    def test_results(_results, slop, fuzzy, top_k):

        results = _results[f"slop_{slop}_fuzzy_{fuzzy}_top_k_{top_k}"]

        for q in queries:
            query_results = []
            for res in index.search(rust_query(q, fuzzy, slop), top_k=top_k):
                r = res.document.content[:100]
                assert r in results.get(
                    q, []
                ), f"Slop: {slop}, fuzzy: {fuzzy}, top-k: {top_k} Result: {r} was returned for query: {q} but isn't present in defined results"

                query_results.append(r)

            for r in results.get(q, []):
                assert (
                    r in query_results
                ), f"Slop: {slop}, fuzzy: {fuzzy}, top-k: {top_k} Result: {r} for query: {q} is missing"

                query_results.append(r)

    _results = {}
    for top_k in [0, 5, 10]:
        for slop in range(0, 4):
            for fuzzy in range(0, 3):
                _results[f"slop_{slop}_fuzzy_{fuzzy}_top_k_{top_k}"] = test_results(
                    results, slop=slop, fuzzy=fuzzy, top_k=top_k
                )
