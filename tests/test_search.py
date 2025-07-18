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
    with open("tests/assets/results_1k.json", "r+") as f:
        data = json.load(f)

    return data


def test_performance(data, queries, results):

    search = MiniSearch()
    search.add("wikipedia")

    for d in data:
        search.index("wikipedia").add(d)

    def test_results(results, slop, fuzzy, score):
        results = results[f"slop_{slop}_fuzzy_{fuzzy}_score_{score}"]

        for q in queries:
            query_results = []
            for r in search.index("wikipedia").search(
                q, slop=slop, fuzzy=fuzzy, score=score
            ):
                r = r["content"][:100]
                assert r in results.get(
                    q, []
                ), f"Result: {r} was returned for query: {q} but isn't present in defined results"

                query_results.append(r)

            for r in results.get(q, []):
                assert r in query_results, f"Result: {r} for query: {q} is missing"

                query_results.append(r)

    for slop in range(0, 4):
        for fuzzy in range(0, 3):
            test_results(results, slop=slop, fuzzy=fuzzy, score=True)
            test_results(results, slop=slop, fuzzy=fuzzy, score=False)
