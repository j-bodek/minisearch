use crate::intersect::PostingListIntersection;
use crate::mis::MinimalIntervalSemanticMatch;
use crate::parser::Query;
use crate::scoring::{bm25, term_bm25};
use crate::tokenizer::Tokenizer;
use crate::trie::Trie;
use hashbrown::HashMap;
use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;
use std::cmp::{Ordering, Reverse};
use std::collections::BinaryHeap;
use std::vec::Vec;
use ulid::{Generator, Ulid};

pub struct Posting {
    pub doc_id: Ulid,
    pub positions: Vec<u32>,
    pub score: f64,
}

pub struct Document {
    pub tokens_num: u32,
    pub content: String,
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
    index: HashMap<String, Vec<Posting>>,
    documents: HashMap<Ulid, Document>,
    ulid_generator: Generator,
    tokenizer: Tokenizer,
    fuzzy_trie: Trie,
    avg_doc_len: f64,
}

#[pymethods]
impl Index {
    #[new]
    fn new() -> Self {
        let mut fuzzy_trie = Trie::new();
        for i in 0..3 {
            fuzzy_trie.init_automaton(i);
        }

        Self {
            index: HashMap::new(),
            documents: HashMap::new(),
            ulid_generator: Generator::new(),
            tokenizer: Tokenizer::new(),
            fuzzy_trie: fuzzy_trie,
            avg_doc_len: 0.0,
        }
    }

    fn add(&mut self, doc: String) -> String {
        let doc_id = self.ulid_generator.generate().unwrap();
        let (tokens_num, tokens) = self.tokenizer.tokenize_doc(doc.clone());

        self.avg_doc_len = (self.avg_doc_len * self.documents.len() as f64 + tokens_num as f64)
            / (self.documents.len() as f64 + 1.0);
        self.documents.insert(
            doc_id,
            Document {
                tokens_num: tokens_num,
                content: doc,
            },
        );

        for (token, positions) in tokens {
            self.fuzzy_trie.add(&token);
            let posting = Posting {
                doc_id: doc_id,
                score: term_bm25(
                    positions.len() as u64,
                    self.documents.len() as u64,
                    self.index.entry_ref(&token).or_default().len() as u64 + 1,
                    tokens_num,
                    self.avg_doc_len,
                ),
                positions: positions,
            };
            self.index.entry_ref(&token).or_default().push(posting);
        }

        doc_id.to_string()
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

        let doc = match self.documents.remove(&id) {
            Some(d) => d,
            _ => return Ok(true),
        };

        let (_, tokens) = self.tokenizer.tokenize_doc(doc.content.clone());
        for (token, _) in tokens {
            let docs = self.index.get_mut(&token).unwrap();
            match docs.binary_search_by(|p| p.doc_id.cmp(&id)) {
                Ok(idx) => {
                    docs.remove(idx);
                }
                _ => (),
            };

            if docs.len() == 0 {
                self.index.remove(&token);
                self.fuzzy_trie.delete(token);
            }
        }

        Ok(true)
    }

    fn search(&mut self, mut query: String, top_k: u8) -> PyResult<Vec<(f64, String, String)>> {
        let query = match Query::parse(&mut query) {
            Err(e) => return Err(e),
            Ok(q) => q,
        };

        let slop = query.slop;
        let query = self.tokenizer.tokenize_query(query);

        let intersection = match PostingListIntersection::new(query, &self.index, &self.fuzzy_trie)
        {
            Some(iter) => iter,
            _ => return Ok(vec![]),
        };

        let mut results = BinaryHeap::with_capacity(top_k as usize);

        for pointers in intersection {
            let (doc_id, mut score) = (pointers[0][0].doc_id, 0.0);
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
                    self.documents.get(&r.0.doc_id).unwrap().content.clone(),
                )
            })
            .collect())
    }
}
