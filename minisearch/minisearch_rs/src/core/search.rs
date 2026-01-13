use crate::analysis::tokenizer::Tokenizer;
use crate::core::index::{IndexManager, Posting};
use crate::errors::{BincodePersistenceError, UlidDecodeError, UlidMonotonicError};
use crate::matching::intersect::PostingListIntersection;
use crate::matching::mis::MinimalIntervalSemanticMatch;
use crate::query::parser::Query;
use crate::query::scoring::{bm25, max_bm25};
use crate::storage::documents::{Document, DocumentsManager};
use crate::utils::hasher::TokenHasher;
use crate::utils::trie::Trie;
use bincode::{Decode, Encode};
use hashbrown::HashSet;
use pyo3::exceptions::{PyKeyError, PyValueError};
use pyo3::prelude::*;
use std::cmp::{Ordering, Reverse};
use std::collections::BinaryHeap;
use std::fs::{self, File};
use std::path::PathBuf;
use std::time::SystemTime;
use std::vec::Vec;
use thiserror::Error;
use ulid::{Generator, MonotonicError, Ulid};

static SEARCH_META_OPERATIONS_THRESHOLD: u32 = 100_000;
static SAVE_META_SECS_THRESHOLD: u64 = 10;

#[derive(Error, Debug)]
enum UlidError {
    #[error("ulid generator: monotonic error: {0}")]
    UlidMonotonicError(#[from] MonotonicError),
    #[error("ulid parse: decode failed: {0}")]
    UlidDecodeError(#[from] ulid::DecodeError),
}

impl From<UlidError> for pyo3::PyErr {
    fn from(err: UlidError) -> Self {
        match err {
            UlidError::UlidMonotonicError(err) => UlidMonotonicError::new_err(err.to_string()),
            UlidError::UlidDecodeError(err) => UlidDecodeError::new_err(err.to_string()),
        }
    }
}

#[derive(Decode, Encode, PartialEq, Debug, Clone)]
struct SearchMetaData {
    avg_doc_len: f64,
}

struct SearchMeta {
    path: PathBuf,
    operations: u32,
    last_save: u64,
    data: SearchMetaData,
}

impl SearchMeta {
    fn new(path: PathBuf) -> Result<Self, BincodePersistenceError> {
        Ok(Self {
            path: path,
            operations: 0,
            last_save: SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)?
                .as_secs(),
            data: SearchMetaData { avg_doc_len: 0.0 },
        })
    }

    fn load(path: PathBuf) -> Result<Self, BincodePersistenceError> {
        if !fs::exists(&path)? {
            File::create(&path)?;
            return Ok(Self::new(path)?);
        }

        let mut file = File::open(&path)?;
        let data: SearchMetaData = if file.metadata()?.len() > 0 {
            bincode::decode_from_std_read(&mut file, bincode::config::standard())?
        } else {
            SearchMetaData { avg_doc_len: 0.0 }
        };

        Ok(Self {
            path: path,
            operations: 0,
            last_save: SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)?
                .as_secs(),
            data: data,
        })
    }

    fn update_avg_doc_len(
        &mut self,
        docs_num: usize,
        new_doc_len: u32,
    ) -> Result<(), BincodePersistenceError> {
        self.data.avg_doc_len = (self.data.avg_doc_len * docs_num as f64 + new_doc_len as f64)
            / (docs_num as f64 + 1.0);

        let cur_ts = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)?
            .as_secs();

        self.operations += 1;

        if self.operations >= SEARCH_META_OPERATIONS_THRESHOLD
            || cur_ts >= self.last_save + SAVE_META_SECS_THRESHOLD
        {
            self.flush()?;
            self.operations = 0;
            self.last_save = cur_ts;
        };

        Ok(())
    }

    fn flush(&self) -> Result<(), BincodePersistenceError> {
        let mut file = File::create(&self.path)?;
        bincode::encode_into_std_write(&self.data, &mut file, bincode::config::standard())?;
        Ok(())
    }
}

pub struct SearchResult {
    pub doc_id: Ulid,
    pub score: f64,
}

impl Ord for SearchResult {
    fn cmp(&self, other: &Self) -> Ordering {
        self.score.total_cmp(&other.score)
    }
}

impl PartialOrd for SearchResult {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.score.total_cmp(&other.score))
    }
}

impl PartialEq for SearchResult {
    fn eq(&self, other: &Self) -> bool {
        self.score == other.score
    }
}

impl Eq for SearchResult {}

#[pyclass(name = "Search")]
pub struct Search {
    index_manager: IndexManager,
    documents_manager: DocumentsManager,
    deleted_documents: HashSet<Ulid>,
    ulid_generator: Generator,
    tokenizer: Tokenizer,
    hasher: TokenHasher,
    fuzzy_trie: Trie,
    meta: SearchMeta,
}

#[pymethods]
impl Search {
    #[new]
    fn new(dir: PathBuf) -> PyResult<Self> {
        let mut fuzzy_trie = Trie::new();
        for i in 0..3 {
            fuzzy_trie.init_automaton(i);
        }

        let hasher = match TokenHasher::load(&dir) {
            Ok(hasher) => hasher,
            Err(e) => {
                return Err(PyValueError::new_err(format!(
                    "Failed to load TokenHasher {}",
                    e.to_string()
                )));
            }
        };
        for token in hasher.tokens() {
            fuzzy_trie.add(token);
        }

        Ok(Self {
            index_manager: IndexManager::load(&dir)?,
            meta: SearchMeta::load(dir.join("meta"))?,
            hasher: hasher,
            documents_manager: DocumentsManager::load(dir)?,
            deleted_documents: HashSet::with_capacity(100),
            ulid_generator: Generator::new(),
            tokenizer: Tokenizer::new(),
            fuzzy_trie: fuzzy_trie,
        })
    }

    fn add(&mut self, mut doc: String) -> PyResult<String> {
        let doc_id = match self.ulid_generator.generate() {
            Ok(id) => id,
            Err(err) => return Err(UlidError::UlidMonotonicError(err).into()),
        };

        let (tokens_num, tokens_map) = self.tokenizer.tokenize_doc(&mut doc);

        self.meta
            .update_avg_doc_len(self.documents_manager.docs.len(), tokens_num)?;

        let mut tokens = Vec::with_capacity(tokens_num as usize);
        for (token, positions) in tokens_map {
            self.fuzzy_trie.add(&token);
            let token = self.hasher.add(token)?;
            let posting = Posting {
                doc_id: doc_id.0,
                positions: positions,
            };
            self.index_manager.insert(token, posting)?;

            tokens.push(token);
        }

        self.documents_manager.write(doc_id, tokens, &doc)?;

        Ok(doc_id.to_string())
    }

    fn get(&self, id: String) -> PyResult<Document> {
        let id = match Ulid::from_string(&id) {
            Ok(val) => val,
            Err(e) => return Err(UlidError::UlidDecodeError(e).into()),
        };

        let doc = match self.documents_manager.docs.get(&id) {
            Some(doc) => doc,
            None => {
                return Err(PyKeyError::new_err(format!(
                    "Document with id: {} does not exist",
                    id,
                )));
            }
        };

        Ok(doc.clone())
    }

    fn delete(&mut self, id: String) -> PyResult<bool> {
        let id = match Ulid::from_string(&id) {
            Ok(val) => val,
            Err(e) => return Err(UlidError::UlidDecodeError(e).into()),
        };

        self.deleted_documents.insert(id);
        self.documents_manager.delete(&id)?;

        if self.deleted_documents.len() >= self.documents_manager.docs.len() / 20 // if greater then 5% of all documents
            || self.deleted_documents.len() <= 1000
        {
            return Ok(true);
        }

        self.force_delete()
    }

    fn search(&mut self, mut query: String, top_k: u32) -> PyResult<Vec<(f64, Document)>> {
        let query = Query::parse(&mut query)?;

        let slop = query.slop;
        let query = self.tokenizer.tokenize_query(query);

        let intersection = match PostingListIntersection::new(
            query,
            &self.index_manager.index,
            &self.hasher,
            &self.fuzzy_trie,
        ) {
            Some(iter) => iter,
            _ => return Ok(vec![]),
        };

        let mut results: BinaryHeap<Reverse<SearchResult>> =
            BinaryHeap::with_capacity(top_k as usize);

        for pointers in intersection {
            let (doc_id, mut score) = (pointers[0][0].doc_id, 0.0);
            if self.deleted_documents.contains(&doc_id) {
                continue;
            }

            let max_score = max_bm25(
                &self.documents_manager,
                self.meta.data.avg_doc_len,
                &pointers,
            );

            if top_k != 0
                && results.len() == top_k as usize
                && let Some(peek) = results.peek()
                && peek.0.score >= max_score
            {
                // skip minimal interval sematic match for non compatative documents
                continue;
            }

            for mis_result in
                MinimalIntervalSemanticMatch::new(&self.index_manager.index, pointers, slop as i32)
            {
                let doc = match self.documents_manager.docs.get(&doc_id) {
                    Some(doc) => doc,
                    None => continue,
                };

                score = bm25(
                    self.documents_manager.docs.len() as u64,
                    doc.tokens.len() as u32,
                    self.meta.data.avg_doc_len,
                    &self.index_manager.index,
                    mis_result,
                )
                .max(score);
            }

            if score > 0.0 {
                if top_k == 0 || results.len() < top_k as usize {
                    results.push(Reverse(SearchResult {
                        doc_id: doc_id,
                        score: score,
                    }));
                } else if let Some(peek) = results.peek()
                    && peek.0.score < score
                {
                    let _ = results.pop();
                    results.push(Reverse(SearchResult {
                        doc_id: doc_id,
                        score: score,
                    }));
                }
            }
        }

        Ok(results
            .into_sorted_vec()
            .into_iter()
            .filter_map(|r| {
                if let Some(doc) = self.documents_manager.docs.get(&r.0.doc_id) {
                    Some((r.0.score, doc.clone()))
                } else {
                    None
                }
            })
            .collect())
    }

    fn flush(&mut self) -> PyResult<()> {
        self.force_delete()?;
        self.documents_manager.flush()?;
        self.index_manager.flush()?;
        self.hasher.flush()?;
        self.meta.flush()?;
        Ok(())
    }

    fn merge(&mut self) -> PyResult<()> {
        self.documents_manager.merge()?;
        Ok(())
    }
}

impl Search {
    fn force_delete(&mut self) -> PyResult<bool> {
        let mut tokens = HashSet::new();
        for d_id in self.deleted_documents.iter() {
            if let Some(doc) = self.documents_manager.docs.remove(d_id) {
                tokens.extend(doc.tokens);
            }
        }

        self.index_manager.delete(
            &tokens,
            &self.deleted_documents,
            &mut self.fuzzy_trie,
            &mut self.hasher,
        )?;

        self.deleted_documents.drain();

        Ok(true)
    }
}
