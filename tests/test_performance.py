import json
import timeit
import pytest
import statistics
from minisearch import MiniSearchRs as MiniSearch

# from whoosh.index import create_in
# from whoosh.fields import *
# from whoosh.qparser import QueryParser, FuzzyTermPlugin
# from whoosh.query import FuzzyTerm


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
    search.add("wikipedia", "data")

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
                        rust_query(q, fuzzy, slop), top_k=0
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

#     def time_queries(slop, fuzzy):

#         class MyFuzzyTerm(FuzzyTerm):
#             def __init__(
#                 self,
#                 fieldname,
#                 text,
#                 boost=0,
#                 maxdist=fuzzy,
#                 prefixlength=0,
#                 constantscore=True,
#             ):
#                 super(MyFuzzyTerm, self).__init__(
#                     fieldname, text, boost, maxdist, prefixlength, constantscore
#                 )

#         with ix.searcher() as searcher:

#             times = []
#             parser = QueryParser("content", ix.schema)
#             parser.add_plugin(FuzzyTermPlugin())

#             for q in queries:

#                 if slop:
#                     q = '"{}"~{}'.format(q, slop)
#                 elif fuzzy:
#                     q = " ".join([f"{w}~{fuzzy}" for w in q.split()])

#                 query = parser.parse(q)
#                 times.append(timeit.timeit(lambda: searcher.search(query), number=1))

#             print(f"FULL TIME: {sum(times)}")
#             print(f"MIN TIME: {min(times)}")
#             print(f"MAX TIME: {max(times)}")
#             print(f"AVG TIME: {statistics.mean(times)}\n")

#     for slop in range(0, 4):
#         print(f"SLOP: {slop}")
#         time_queries(slop=slop, fuzzy=0)

#     for fuzzy in range(0, 3):
#         print(f"FUZZY: {fuzzy}")
#         time_queries(slop=0, fuzzy=fuzzy)
