import os
import time
import json
import shutil
import pytest
from minisearch import MiniSearch

MINISEARCH_DIR = "data"


# helper functions
def rust_query(query, fuzzy, slop):
    return '"' + " ".join([f"{t}~{fuzzy}" for t in query.lower().split()]) + f'"~{slop}'


def validate_results(index, queries, _results, slop, fuzzy, top_k):

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


def validate_all_results(top_ks, slops, fuzzies, index, queries, results):
    for top_k in top_ks:
        for slop in slops:
            for fuzzy in fuzzies:
                validate_results(
                    index,
                    queries,
                    results,
                    slop=slop,
                    fuzzy=fuzzy,
                    top_k=top_k,
                )


@pytest.fixture
def queries():
    queries = []
    with open("tests/assets/queries.txt", "r+") as f:
        for q in f.readlines():
            queries.append(q.strip())

    return queries


@pytest.fixture
def data():
    def func(dir_name):
        files = {
            "test_deletes": ["articles.json", "deletes.json"],
            "test_regular": ["articles.json"],
        }

        files = files[dir_name]

        data = []
        for f in files:
            with open(f"tests/assets/{dir_name}/{f}", "r+") as f:
                data.append(list(json.load(f).values()))

        return data[0] if len(data) == 1 else data

    return func


@pytest.fixture
def results():
    def func(dir_name):
        files = {
            "test_deletes": "results.json",
            "test_regular": "results.json",
        }

        file = files[dir_name]
        with open(f"tests/assets/{dir_name}/{file}", "r+") as f:
            data = json.load(f)

        return data

    return func


#  after each test cleanup
@pytest.fixture(autouse=True)
def cleanup():
    yield
    if os.path.exists(MINISEARCH_DIR):
        shutil.rmtree(MINISEARCH_DIR)


def test_search(subtests, data, queries, results):
    with subtests.test(msg="test_search [new data]"):

        data, results = data("test_regular"), results("test_regular")

        search = MiniSearch()
        s = time.time()
        _, index = search.add("wikipedia", MINISEARCH_DIR)
        print(f"Loading took: {time.time() - s}")

        s = time.time()
        with index.session():
            for d in data:
                index.add(d)

        print(f"Inserting took: {time.time() - s}")

        validate_all_results(
            [0, 5, 10], range(0, 4), range(0, 3), index, queries, results
        )

    with subtests.test(msg="test_search [new data]"):
        # load persisted data
        s = time.time()
        search = MiniSearch()
        _, index = search.add("wikipedia", MINISEARCH_DIR)
        print(f"Loading took: {time.time() - s}\n\n")

        validate_all_results(
            [0, 5, 10], range(0, 4), range(0, 3), index, queries, results
        )


def test_search_after_deletes(subtests, data, queries, results):

    with subtests.test(msg="test_search_after_deletes [new data]"):

        (data, deletes), results = data("test_deletes"), results("test_deletes")

        search = MiniSearch()
        _, index = search.add("wikipedia", MINISEARCH_DIR)

        to_delete = []
        s = time.time()
        with index.session():
            for d in data:
                index.add(d)

            for d in deletes:
                to_delete.append(index.add(d))

        with index.session():
            for _id in to_delete:
                index.delete(_id)

        print(f"Insert and delete took: {time.time() - s}\n\n")

        validate_all_results(
            [0, 5, 10], range(0, 4), range(0, 3), index, queries, results
        )

    with subtests.test(msg="test_search_after_deletes [persisted data]"):

        # load persisted data
        s = time.time()
        search = MiniSearch()
        _, index = search.add("wikipedia", MINISEARCH_DIR)
        print(f"Loading took: {time.time() - s}\n\n")

        validate_all_results(
            [0, 5, 10], range(0, 4), range(0, 3), index, queries, results
        )


def test_search_after_merge(subtests, data, queries, results):

    with subtests.test(msg="test_search_after_merge [new data]"):
        (data, deletes), results = data("test_deletes"), results("test_deletes")

        search = MiniSearch()
        _, index = search.add(
            "wikipedia", MINISEARCH_DIR, "tests/assets/merge_test_conf.toml"
        )

        s = time.time()
        to_delete = []
        with index.session():
            for d in data:
                index.add(d)

            for d in deletes:
                to_delete.append(index.add(d))

        with index.session():
            for _id in to_delete:
                index.delete(_id)

            index.merge()

        print(f"Insert, delete and merge took: {time.time() - s}\n\n")

        validate_all_results(
            [0, 5, 10], range(0, 4), range(0, 3), index, queries, results
        )

    with subtests.test(msg="test_search_after_merge [persisted data]"):
        # load persisted data
        s = time.time()
        search = MiniSearch()
        _, index = search.add("wikipedia", MINISEARCH_DIR)
        print(f"Loading took: {time.time() - s}\n\n")

        validate_all_results(
            [0, 5, 10], range(0, 4), range(0, 3), index, queries, results
        )
