## Theory of operation

This document briefly desscribes how minisearch works under the hood. It serves the purpose of better understanding machinsm and also like a learning material for getting the picture how search engines works.


### Inverted index - the main datastructure behind search engine

The main datastructure behind minisearch is inverted index. This amazing datastructure enables to efficiently find all documents containing the given token by mapping the token to their postings list. Posting list is nothing more then collection of all documents containing the token to which the posting list is mapped to with positions of token apperance in those documents. Such structure allows for efficient retrival of documents that match the query tokens and then finding the documents that match that query.


### Unicode text segmantation and Snowball stemmer - turning documents into tokens

Unicode text segmentation is used for retriving the words from the documents. It allows to brake the documents into smaller units. After unicode segmentation each extracted word is injected into snowball stemmer. Snowball stemmer is stemming algorithm which purpose is to get the stem form of the word. The stemming is important because get the stem for of the word for example reading -> read, reads -> read etc. By doing so queries are more efficient, inverted index contains less keys and the search works better because it don't care about term form it only cares if it appears or not. Such transformed terms are then called tokens, for each document all tokens are extracted, stop words (such as 'a', 'and' etc.) are removed because they are not meaningfull from search perspective and positions of those tokens are grouped by token and stored in inverted index.


### Query parsing - parsing query with custom parser

Query parser is defined with chumsky - high-performance parsing library. The syntax definition is really straight forward. Each non-valid queries result in meaningfull error displayed to the end user.


### Levenshtein automaton - fast retrieval of similar tokens

Fuzzy search is the type of search that supports detecting the documents with taking into consideration typo in query or document itself. It can be achieved by comparing the tokens similarity to check if they are similar enough. The similarity is meassured by text similarity algorithms, Minisearch uses well known similarity algorithm called Levenshtein Distance. It meassures minimal number of operations (where operation
is deletion of character, addition of character or replacement of character) that are needed to transform one string into another. It works great but with brute force approach - iterating over tokens from inverted index and comparing them with token from query - isn't perfect and can take significant amount of time especially
with larger inverted-indexes. For searching all similar strings within distance of N from the set of strings there is way more optimal approach, for that we can use Levenshtain DFA (deterministic finite automaton). The dfa is state machine that:
- is finite - has it's final accept/reject state
- deterministic - each state has transition to at most one other state

It is defined for the string for which we want to check if other string(s) is similar and then to check similarity we feed it with symbols from other string one by one. Automaton returns the next state for each given symbol. This is extremally powerfull for searching all similar strings within set because the set of the words can be transformed into Trie and then the trie can be traversed and each symbol during traversal inserted into Levenshtain DFA, if the distance is greater then the given one then we don't traverse deeper get back to previous node and continue traversing other nodes. By doing that we are skipping huge parts of the trie and just check subset of actual words from the set which result in much better computation time.


### Bm25 - scoring the final results

For calculating the score for the documents included in results the bm25 function is used. Final document score is calculated by evaluating the score for each query token and then summing them together. Token score takes into account things like TF (time frequecy) - number a token appeared in document and IDF (inverse document frequency) - measurement that tells how rare token is amongs all of the documents.



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