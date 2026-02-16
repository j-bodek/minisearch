# Minisearch

Minisearch is a lightweight, local full-text search library for Python. It provides a simple API for indexing text and running ranked searches without external services or infrastructure.

## Documentation

### Query language

Queries are parsed as phrases; term order matters even without quotes. Each term can include optional fuzziness, and phrases can include a slop value.

**Examples**

- Phrase query: `python search` or `"python search"`
  returns: documents where the terms appear as an exact phrase.

- Term with fuzziness: `pyth~1 search~2`
  returns: documents that match the phrase in order, allowing per-term fuzziness.

- Phrase with slop: `"full text search"~2`
  returns: documents where the phrase terms appear within a slop window of 2 positions.

Notes:
- Fuzziness (`~N`) applies to individual terms and allows minor spelling differences.
- Slop (`"..."~N`) applies to phrases and allows terms to appear within N positions of each other.
- Fuzziness is limited to 0-2 per term.
- Slop is limited to 0-99.

### Adding/Deleting indexes

An index is a named handle that points at an on-disk directory. You can create multiple indexes in one process using `MiniSearch`.

```python
from minisearch import MiniSearch

search = MiniSearch()
created, index = search.add("wikipedia", "./data")

# remove handle only (does not delete data on disk)
search.delete("wikipedia")
```

`MiniSearch.add()` returns `(created, index)` where `created` is `True` if the index was created for the first time in the current process.

### Adding, deleting, getting documents

Documents are added as strings and are tokenized, stemmed, and indexed.

```python
from minisearch import MiniSearch

search = MiniSearch()
_, index = search.add("demo", "./data")

with index.session():
    doc_id = index.add("The quick brown fox")
    index.add("The quick brown fox jumps")

# fetch by ULID
_doc = index.get(doc_id)
```

Best practice: use `index.session()` when inserting or deleting many documents. The session ensures buffered data is flushed to disk on exit, reducing the risk of data loss if the process stops before an explicit `flush()`.

### Running queries

`Index.search()` returns a list of results ordered by score. Each result contains a score and a document. Document content is fetched lazily.

```python
results = index.search("\"quick fox\"~1", top_k=5)
for r in results:
    print(r.score, r.document.content)
```

`top_k=0` returns all matches; otherwise the results are capped to the top K.

### Flush and merge

`flush()` persists buffered writes (documents, index logs, metadata). It is automatically called when a session exits.

`merge()` compacts old segments by removing deleted documents. Use it after many deletes to reclaim space and improve search speed.

Caveat: because documents are loaded lazily, a merge can invalidate previously fetched `Document` handles that still point at old segment locations. Fetch new documents after a merge.

### Settings

Minisearch can be configured via a TOML file passed to `Index` / `MiniSearch`.

```python
search = MiniSearch(conf="./minisearch.toml")
_, index = search.add("demo", "./data")
```

Example `minisearch.toml`:

```toml
# segment storage size
segment_size = 52428800
# buffer size before flush
documents_buffer_size = 1048576
# seconds before auto flush
documents_save_after_seconds = 5
# segment compaction threshold
merge_deleted_ratio = 0.3

# search metadata
metadata_save_after_operations = 100000
metadata_save_after_seconds = 10

# index logs
index_buffer_size = 1048576
index_save_after_operations = 100000
index_save_after_seconds = 5

# custom stop words
stop_words = ["a", "the", "and"]
```

All fields are optional; missing fields use defaults.
