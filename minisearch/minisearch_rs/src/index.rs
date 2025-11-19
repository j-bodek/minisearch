use crate::intersect::PostingListIntersection;
use crate::mis::MinimalIntervalSemanticMatch;
use crate::parser::Query;
use crate::scoring::{bm25, term_bm25};
use crate::tokenizer::Tokenizer;
use crate::trie::Trie;
use hashbrown::HashMap;
use pyo3::prelude::*;
use std::vec::Vec;
use ulid::{Generator, Ulid};

pub struct Posting {
    pub doc_id: Ulid,
    pub positions: Vec<u32>,
    pub score: f64,
}

#[pyclass(name = "Index")]
pub struct Index {
    index: HashMap<String, Vec<Posting>>,
    documents: HashMap<Ulid, u32>,
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
        let (tokens_num, tokens) = self.tokenizer.tokenize_doc(doc);

        self.avg_doc_len = (self.avg_doc_len * self.documents.len() as f64 + tokens_num as f64)
            / (self.documents.len() as f64 + 1.0);
        self.documents.insert(doc_id, tokens_num);

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

    fn search(&mut self, mut query: String, top_k: u8) -> PyResult<()> {
        // get slop, trim all of the '"' and white spaces

        let query = match Query::parse(&mut query) {
            Err(e) => return Err(e),
            Ok(q) => q,
        };

        let slop = query.slop;
        let query = self.tokenizer.tokenize_query(query);

        let intersection = match PostingListIntersection::new(query, &self.index, &self.fuzzy_trie)
        {
            Some(iter) => iter,
            _ => return Ok(()),
        };

        for pointers in intersection {
            let doc_id = pointers[0][0].doc_id;
            for mis_result in MinimalIntervalSemanticMatch::new(&self.index, pointers, slop as i32)
            {
                let score = bm25(
                    self.documents.len() as u64,
                    *self.documents.get(&doc_id).unwrap_or(&0),
                    self.avg_doc_len,
                    &self.index,
                    mis_result,
                );

                println!("doc_id: {}, score: {}", doc_id, score);
            }
        }

        Ok(())
    }
}
