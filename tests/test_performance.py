import json
import timeit
import pytest
import statistics
from minisearch import MiniSearch


@pytest.fixture
def data():
    with open("tests/assets/articles_50k.json", "r+") as f:
        data = json.load(f)

    return data.values()


@pytest.fixture
def queries():
    queries = []
    with open("tests/assets/queries.txt", "r+") as f:
        for q in f.readlines():
            queries.append(q.strip())

    return queries


def rust_query(query, fuzzy, slop):
    return '"' + " ".join([f"{t}~{fuzzy}" for t in query.lower().split()]) + f'"~{slop}'


def test_performance(data, queries):

    search = MiniSearch()
    _, index = search.add("wikipedia", "data")

    def insert_articles(data, index):
        def _wrapper():
            with index.session():
                for d in data:
                    index.add(d)

        return _wrapper

    _time = timeit.timeit(insert_articles(data, index), number=1)
    print(f"\nINSERTION TIME OF {len(data)} ARTICLES: {_time}")

    def time_queries(slop, fuzzy, score):
        print(f"QUERIES TIME: slop: {slop}, fuzzy: {fuzzy}, score: {score}")

        times = []

        for q in queries:
            times.append(
                timeit.timeit(
                    lambda: index.search(rust_query(q, fuzzy, slop), top_k=0),
                    number=1,
                )
            )

        print(f"FULL TIME: {sum(times)}")
        print(f"MIN TIME: {min(times)}")
        print(f"MAX TIME: {max(times)}")
        print(f"AVG TIME: {statistics.mean(times)}\n")

    for slop in range(0, 4):
        for fuzzy in range(0, 3):
            time_queries(slop=slop, fuzzy=fuzzy, score=True)
            time_queries(slop=slop, fuzzy=fuzzy, score=False)
