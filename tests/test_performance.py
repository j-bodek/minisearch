import json
import timeit
import pytest
import statistics
from minisearch import MiniSearch
from whoosh.index import create_in
from whoosh.fields import *
from whoosh.qparser import QueryParser
from whoosh.query import FuzzyTerm


@pytest.fixture
def data():
    with open("tests/assets/articles_10k.json", "r+") as f:
        data = json.load(f)

    return data.values()


@pytest.fixture
def queries():
    queries = []
    with open("tests/assets/queries.txt", "r+") as f:
        for q in f.readlines():
            queries.append(q.strip())

    return queries


def test_performance(data, queries):

    search = MiniSearch()
    search.add("wikipedia")

    def insert_articles(data, search):
        def _wrapper():
            for d in data:
                search.index("wikipedia").add(d)

        return _wrapper

    _time = timeit.timeit(insert_articles(data, search), number=1)
    print(f"\nINSERTION TIME OF {len(data)} ARTICLES: {_time}")

    def time_queries(slop, fuzzy, score):
        print(f"QUERIES TIME: slop: {slop}, fuzzy: {fuzzy}, score: {score}")

        times = []

        for q in queries:
            times.append(
                timeit.timeit(
                    lambda: search.index("wikipedia").search(
                        q, slop=slop, fuzzy=fuzzy, score=score
                    ),
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


# def test_performance_whoosh(data, queries):

#     schema = Schema(content=TEXT(stored=True))
#     ix = create_in("whoosh_indexdir", schema)

#     def insert_articles(data, index):
#         def _wrapper():

#             writer = ix.writer()
#             for d in data:
#                 writer.add_document(content=d)

#             writer.commit()

#         return _wrapper

#     _time = timeit.timeit(insert_articles(data, ix), number=1)
#     print(f"\nINSERTION TIME OF {len(data)} ARTICLES: {_time}")

#     with ix.searcher() as searcher:
#         for q in queries:
#             query = QueryParser("content", ix.schema, termclass=FuzzyTerm).parse(q)
#             # query = QueryParser("content", ix.schema).parse(q)
#             print(f"QUERY: {q}")
#             for r in searcher.search(query):
#                 # pass
#                 print(f"- RESULT: {r.fields()['content'][:100]}")
#                 # has_result = True
#             break
