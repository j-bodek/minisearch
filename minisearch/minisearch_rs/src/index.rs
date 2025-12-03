use crate::intersect::PostingListIntersection;
use crate::mis::MinimalIntervalSemanticMatch;
use crate::parser::Query;
use crate::scoring::{bm25, term_bm25};
use crate::tokenizer::Tokenizer;
use crate::trie::Trie;
use crate::utils::hasher::TokenHasher;
use crate::utils::writer::{DocLocation, DocumentsWriter};
use hashbrown::{HashMap, HashSet};
use pyo3::exceptions::{PyKeyError, PyValueError};
use pyo3::prelude::*;
use std::cmp::{Ordering, Reverse};
use std::collections::BinaryHeap;
use std::path::PathBuf;
use std::vec::Vec;
use ulid::{Generator, Ulid};

pub struct Posting {
    pub doc_id: Ulid,
    pub positions: Vec<u32>,
    pub score: f64,
}

pub struct Document {
    pub tokens_num: u32,
    pub location: DocLocation,
    pub tokens: HashSet<u32>,
}

pub struct Result {
    pub doc_id: Ulid,
    pub score: f64,
}

impl Ord for Result {
    fn cmp(&self, other: &Self) -> Ordering {
        self.score.total_cmp(&other.score)
    }
}

impl PartialOrd for Result {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.score.total_cmp(&other.score))
    }
}

impl PartialEq for Result {
    fn eq(&self, other: &Self) -> bool {
        self.score == other.score
    }
}

impl Eq for Result {}

#[pyclass(name = "Index")]
pub struct Index {
    index: HashMap<u32, Vec<Posting>>,
    documents: HashMap<Ulid, Document>,
    writer: DocumentsWriter,
    deleted_documents: HashSet<Ulid>,
    ulid_generator: Generator,
    tokenizer: Tokenizer,
    hasher: TokenHasher,
    fuzzy_trie: Trie,
    avg_doc_len: f64,
}

#[pymethods]
impl Index {
    #[new]
    fn new(dir: PathBuf) -> PyResult<Self> {
        let mut fuzzy_trie = Trie::new();
        for i in 0..3 {
            fuzzy_trie.init_automaton(i);
        }

        Ok(Self {
            writer: DocumentsWriter::new(dir)?,
            index: HashMap::new(),
            documents: HashMap::new(),
            deleted_documents: HashSet::with_capacity(100),
            ulid_generator: Generator::new(),
            tokenizer: Tokenizer::new(),
            hasher: TokenHasher::new(),
            fuzzy_trie: fuzzy_trie,
            avg_doc_len: 0.0,
        })
    }

    fn add(&mut self, mut doc: String) -> PyResult<String> {
        let doc_id = self.ulid_generator.generate().unwrap();
        let location = self.writer.write(doc_id, &doc)?;

        let (tokens_num, tokens_map) = self.tokenizer.tokenize_doc(&mut doc);

        self.avg_doc_len = (self.avg_doc_len * self.documents.len() as f64 + tokens_num as f64)
            / (self.documents.len() as f64 + 1.0);

        let mut tokens = HashSet::with_capacity(tokens_num as usize);
        for (token, positions) in tokens_map {
            self.fuzzy_trie.add(&token);
            let token = self.hasher.add(token);
            let posting = Posting {
                doc_id: doc_id,
                score: term_bm25(
                    positions.len() as u64,
                    self.documents.len() as u64 + 1,
                    self.index.entry(token).or_default().len() as u64 + 1,
                    tokens_num,
                    self.avg_doc_len,
                ),
                positions: positions,
            };
            self.index.entry(token).or_default().push(posting);
            tokens.insert(token);
        }

        self.documents.insert(
            doc_id,
            Document {
                tokens_num: tokens_num,
                location: location,
                tokens,
            },
        );

        Ok(doc_id.to_string())
    }

    fn get(&self, id: String) -> PyResult<String> {
        let id = match Ulid::from_string(&id) {
            Ok(val) => val,
            Err(e) => {
                return Err(PyValueError::new_err(format!(
                    "Invalid ULID: {}",
                    e.to_string()
                )))
            }
        };

        let doc = match self.documents.get(&id) {
            Some(doc) => doc,
            None => {
                return Err(PyKeyError::new_err(format!(
                    "Document with id: {} does not exist",
                    id,
                )))
            }
        };

        match self.writer.read(doc) {
            Ok(val) => return Ok(val),
            Err(e) => {
                return Err(PyValueError::new_err(format!(
                    "Error while reading document: {}",
                    e.to_string()
                )))
            }
        };
    }

    fn delete(&mut self, id: String) -> PyResult<bool> {
        let id = match Ulid::from_string(&id) {
            Ok(val) => val,
            Err(e) => {
                return Err(PyValueError::new_err(format!(
                    "Invalid ULID: {}",
                    e.to_string()
                )))
            }
        };

        if !self.documents.contains_key(&id) {
            return Ok(true);
        }

        self.deleted_documents.insert(id);
        self.writer.delete(id)?;
        if self.deleted_documents.len() >= self.documents.len() / 20 // if greater then 5% of all documents
            || self.deleted_documents.len() <= 1000
        {
            return Ok(true);
        }

        let mut tokens = HashSet::new();
        for d_id in self.deleted_documents.iter() {
            if let Some(doc) = self.documents.remove(d_id) {
                tokens.extend(doc.tokens);
            }
        }

        for token in tokens {
            let docs = match self.index.get_mut(&token) {
                Some(docs) => docs,
                _ => continue,
            };

            docs.retain(|doc| !self.deleted_documents.contains(&doc.doc_id));

            if docs.len() == 0 {
                self.index.remove(&token);
                self.fuzzy_trie.delete(self.hasher.delete(token).unwrap());
            }
        }

        self.deleted_documents.drain();

        Ok(true)
    }

    fn search(&mut self, mut query: String, top_k: u8) -> PyResult<Vec<(f64, String, String)>> {
        let query = match Query::parse(&mut query) {
            Err(e) => return Err(e),
            Ok(q) => q,
        };

        let slop = query.slop;
        let query = self.tokenizer.tokenize_query(query);

        let intersection = match PostingListIntersection::new(
            query,
            &self.index,
            &self.hasher,
            &self.fuzzy_trie,
        ) {
            Some(iter) => iter,
            _ => return Ok(vec![]),
        };

        let mut results = BinaryHeap::with_capacity(top_k as usize);

        for pointers in intersection {
            let (doc_id, mut score) = (pointers[0][0].doc_id, 0.0);
            if self.deleted_documents.contains(&doc_id) {
                continue;
            }

            for mis_result in MinimalIntervalSemanticMatch::new(&self.index, pointers, slop as i32)
            {
                score = bm25(
                    self.documents.len() as u64,
                    self.documents.get(&doc_id).unwrap().tokens_num,
                    self.avg_doc_len,
                    &self.index,
                    mis_result,
                )
                .max(score);
            }

            if score > 0.0 {
                if top_k == 0 || results.len() < top_k as usize {
                    results.push(Reverse(Result {
                        doc_id: doc_id,
                        score: score,
                    }));
                } else if results.peek().unwrap().0.score < score {
                    let _ = results.pop();
                    results.push(Reverse(Result {
                        doc_id: doc_id,
                        score: score,
                    }));
                }
            }
        }

        Ok(results
            .into_sorted_vec()
            .into_iter()
            .map(|r| {
                (
                    r.0.score,
                    r.0.doc_id.to_string(),
                    // todo, don't read all of the data to memory, lazy load instead (some rust struct that can be returned?)
                    self.writer
                        .read(self.documents.get(&r.0.doc_id).unwrap())
                        .unwrap(), //todo remove this
                )
            })
            .collect())
    }
}
