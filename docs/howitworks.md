## Theory of operation

This document briefly describes how minisearch works under the hood. It serves the purpose of better understanding Minisearch machanism and also serves as a learning material for getting the picture of how search engines works.


### Inverted index - the main datastructure behind search engine

The main datastructure behind Minisearch is inverted index. It enables to efficiently find all documents containing the given term by mapping the terms to their postings list. Posting list is nothing more then collection of all documents containing the term to which the posting list is mapped and also positions of term apperances in those documents. The visual representation of it looks like that

![inverted index](assets/inverted_index.png "inverted index")

Such structure allows for efficient retrival of documents that match the given query.


### Unicode text segmantation and Snowball stemmer - turning documents into tokens

When new document is added to Minisearch it is first analyzed and inserted into inverted index. Main process of document analysis is tokenization which is splitting the document into single unit of informations.

First all unicode words are extracted from document, for this the [unicode text segmentation](https://www.unicode.org/reports/tr29/ "unicode text segmentation") is used. After word extraction there is a step of skipping stop words, these are common, high frequency words that contribute little to the meaning of the sentence for example: "a", "an", "on" etc. After that each word is processed using [Snowball stemming algorithm](https://snowballstem.org/algorithms/english/stemmer.html "Snowball stemming algorithm"). Stemming is the process of unifying words to their single form called stem, for example Snowball Stemmer will map "connecting", "connection", "connective" and "connected" to unified form "connect". By doing so search can find all possible matches containing the word regarding it's form. This also results in smaller inverted index and overall better search performance. Such transformed words are then called tokens, for each document they are extracted with positions they appear in the document and inserted into inverted index.


### Query parsing - parsing query with custom parser

Minisearch query language is failry simple and can be handled by simple logic written with regexps and basic string manipulations. However such approach have few problems:
- error handling - it wouldn't be trivial to return descriptive error messages highlighting exact problem and place in query where it occured
- future developement - if query language grammar will evolve over time maintaining such logic will quickly result in spaghetti parser that is unmaintanable

For those reasons i decided to write small and easy parser with [chumsky](https://crates.io/crates/chumsky "chumsky") library, it offers error handling and simplicity with defining new grammar rules with great performance.


### Levenshtein automaton - fast retrieval of similar tokens

Approximate string matching (aka. fuzzy search) is the type of search that instead of searching document by exact terms given in a query can instead search for terms within specified similarity to the ones given in a query. For example in Minisearch following query will search all documents that contain word 'elephant' within similarity of 2.

```"elephant~2"```

Minisearch measures terms similarity by using Levenshtein Distance. It finds minimal number of operations needed to transform one string into another where operation can be either insertion of the new character, deletion of the character or replacement of the character. For example Levenshtein Distance between "cat" and "call" is 2 because it needs at least two operations to transform either "cat" into "call" or "call" into "cat".

```
- 1 - replace "t" in "cat" with "l" -> "cal"
- 2 - insert "l" in the end of the "cal" -> "call"
```

The fuzzy search uses Levenshtein Distance to find all tokens stored in the inverted index that are within similarity N to the searching token. Brute force approach of doing that will be iterating through all of the tokens stored in inverted index and computing Levenshtein Distance between them and search token which is highly inefficient especially with larger index. Luckily there is a lot better way of doing that.

There is a great [paper](assets/2002_Schulz.pdf "paper") describes Levenshtein DFA (Deterministic Finite Automaton). To understand how this model works first it is important to understand what DFA is, it is state machine that is:
- finite - meaning it has finite number of states
- deterministic - which means that each transition can return exactly one next state

Idea behind levenshtein automaton is to construct generic DFA structure for fixed N that can then be reused to compute Levenshtein Distance of degree N for given string, only the transitions depend on the actual string. It then accepts single character and return new state that is either:
- intermediate - Levenshtein Distance of string constructed from given characters isn't within N but it still can be within Levenshtein Distance of N after receiving new characters
- final:
    - accepting - Levenshtein Distance of string constructed from given characters is <= N
    - dead/rejecting - Levenshtein Distance > N and there is no possibility that inserting new characters will change that

Authors of the paper observed that for automaton of fixed degree N the next state can be computed by analyzing fixed window of string next characters. That observation lead to construction of characteristic vectors that defines where the input character appears in the window, they are then used to make fast decisions about next step transition.

Finding all terms that are within Levenshtein Distance of degree N to the search term with usage of Levenshtein Automaton can be done by using [Trie](https://en.wikipedia.org/wiki/Trie "Trie"). First Trie with all tokens that are stored in inverted index is constructed. Next Trie is traversed and with each step the new character is given to Levenshtein Automaton, automaton then returns state that if:
- intermediate - continue traversal
- final 
    - if acceptance - check if accepted Trie node is a word, if so return it as similar
    - if dead state - stop traversal and get back to previous node

Such technique results in way faster retrival of similar tokens then brute force approach, it checks prefixes of the tokens once and also skip huge portions of Trie by quickly detecting dead states.



### Bm25 - scoring the final results

For calculating the score of the documents included in results the [bm25](https://pl.wikipedia.org/wiki/Okapi_BM25 "bm25") function is used. Final document score is calculated by evaluating the score for each query token and then summing them together. Token score takes into account things like TF (time frequecy) - number a token appeared in document and IDF (inverse document frequency) - measurement that tells how rare token is amongs all of the documents.



### Posting list intersection - retrieving documents containing query tokens

This is the process of retriving all documents that contains tokens from the term - right now without checking any particular order. First for each token from query we get all tokens that are within the given similarity (distance=0 for same token, distance=1, 2 etc.), that's done with levenshtein automaton and tokens trie. After similar tokens are retrieved the pointer heap is constructed, it points to first document in the inverted index for the given token. Such pointer heap is constructed for all similar token groups in query. After that the actual posting list intersection starts, first the initial documents are retrieved from each pointer heap. If all documents returned from the heaps are the same then the document contains all the tokens we want otherwise max token is choosen and for each pointer heap the pointer is moved to document greater or equal to the max document. After moving pointer the document to which the pointer points is returned and the evaluation is repeated. This process last untill any of the heaps is empty.



### Minimal-interval semantics - checking if document match a query

https://vigna.di.unimi.it/ftp/papers/EfficientLazy.pdf

This is a process of actually checking the order of the query tokens in the actual query, right there the query sloppines are evaluated. First the tokens iterator window is constructed, in the posting list intersection for each token there are at least one token within the given similarity found in the document and passed to the minimal-interval semantics. Let's call those similar tokens a token group. Minimal-interval semantics takes vector of tokens groups, for each tokens group the TokenGroupIterator is constructed, it works as the wrapper around all the tokens from the group returning next min position of the given tokens for specific document. It is constructed based on the n-size min-heap where n is the number of the tokens in token group. Such TokensGroupIterator is constructed for each tokens group and stored in the vector representing all the query tokens. After that the minimal-interval semantics calculation is done, for each token group starting from index=1 up to N where N is the number of token groups the closest position that is >= to the position of the previous token is found, slop is evaluated and updated and if it is within the given slop then the process is repeated for the next index. Otherwise the next position for tokens group at index i=0 is found and index is reset to i=1 and all previous process is repeat. If the index i goes up to index=N and the calculated slop is within given sloppiness then interval that meets the requirements is found and returned. All process is run untill any of the token iterators is empty.


### Maxscore - skipping minimal-interval semantics for non-competative documents

Calculating minimal-interval semantics is complicated process that needs allocating some memory and running more complex computations. If the search returns top-k results this process can be skipped for many documents by identifying non competative documents and skipping minimal-interval semantic for them. It can be done by storing the already found results in the heap of size K where K is the number of top-k results. Then for each new document that is classified to run minimal-interval semantics on the max possible bm25 score is computed. If this score is less or equal to the minimal score from the results heap then the minimal-interval semantic is skipped since this document is not competative. Otherwise the minimal-interval semantics is calculated and if it matches and score is greater then the minimal element of results heap then results heap is updated.


### Persistence lifecycle - buffers, compression and AOF logs

Persistence of the Minisearch can be divided into two main categories:
- documents persistance
- index persistance

Let's start with documents persistance, after adding new document to Minisearch it is split into tokens which are inserted into the inverted index. But the actual document isn't stored in the memory, documents are stored in segments - they are binary files stored on a disk. There are multiple segments and each one of them store data up to specified threshold (by default 50MB), that's because of performance reasons like faster seeks on smaller files, better handling that my os etc. Each segment is made out of three binary files:
- data - storing the documents
- meta - storing metadata about the documents
- del - storing deleted documents because data is AOF fle

When writing new document, first it is compressed with lz4 compression algorithm, decision to use it was made because it is extremaly fast and still offers acceptably good compresion. Compressed document is then saved to memory buffer. After that the metadata for the document is created, it stores document id, document tokens, location - segment, offset and size of compressed document. This metadata is encoded into bytes and stored with the u64 prefix annotating it's size in metadata buffer. The document metadata object is also stored in the memory allowing fast document retrival if needed. Then if documents buffer exceeds the given threshold (by default 1MB) or last save was older then the given threshold (by default 5 seconds) then data from buffer is saved into disk.

Deletion of the document is fairly simple, when document is deleted it's id and size is written into del file.

Because data, meta and del files are AOF files no modification of already inserted data are made. Because of that after deleting significant number of documents large number of data stored on disk isn't actually used and can be safely deleted. That's why merge mechanism was introduced. It iterates over all segments and check if ratio of deleted/undeleted documents is greater or equal to a given threshold (by default 30%), if so data is read into memory buffers skipping all of the deleted documents and sequentially written into new segment. After this process is finished old segment is deleted and replaced by new one.

Restoring documents metadata - when initializing Minisearch with existing data, on startup it restores the documents metadata in memory. It iterates over segments and read their metadata by first checking 8 bytes that stores the size of the document metadata object then reading the metadata object to memory and reapeating that process untill it reaches the end of the file.



Index persistance - index persistance is done by storing the logs describing operations made on the inverted index in the AOF files and then reconstructing them. Index is stored in three binary files:
- index - logs add/delete on inverted index
- meta - logs metadata
- tokens - maps that map tokens to u32 and u32 back to string

Updating inverted index - when updating inverted index all insert/delete operations are written as logs into buffer. They are encoded into binary format and saved into memory, after buffer size exceeds threshold (by default 1MB) or last save was older then the threshold (by default 5 seconds) then logs are appended to index file. For each log the associated log meta is created, it is of fixed size and the binary serialization looks like this:

doc_id:16 bytes|offset:8 bytes|size:4 bytes

Each log however has different size and stores following informations:

- operation - either ADD or DELETE
- token - u32 representing the inverted index token that was modified
- postings_num - number of postings associated with token after the operation
- posting - only for ADD log - posting that was added to inverted index


Storing this informations and metadata of fixed size allows to reconstruct the index starting from the latest operation which allows to allocate the proper amount of memory with advance and skip insertion of documents that are deleted afterward.