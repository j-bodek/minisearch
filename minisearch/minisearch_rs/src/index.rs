use crate::parser::Query;
use crate::scoring::term_bm25;
use crate::tokenizer::Tokenizer;
use crate::trie::Trie;
use chumsky::prelude::*;
use hashbrown::HashMap;
use pyo3::prelude::*;
use std::vec::Vec;
use ulid::{Generator, Ulid};

struct Posting {
    doc_id: Ulid,
    positions: Vec<u32>,
    score: f64,
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
            let posting = Posting {
                doc_id: doc_id,
                score: term_bm25(
                    positions.len() as u64,
                    self.documents.len() as u64,
                    self.index.entry_ref(&token).or_default().len() as u64 + 1,
                    tokens_num as u64,
                    self.avg_doc_len,
                ),
                positions: positions,
            };
            self.index.entry_ref(&token).or_default().push(posting);
        }

        doc_id.to_string()
    }

    fn search(&self, q: String, top_k: u8) -> PyResult<()> {
        // get slop, trim all of the '"' and white spaces

        let qr = Query::parse(&q);
        match qr {
            Err(e) => return Err(e),
            _ => (),
        };

        Ok(())
        // let errors = qr
        //     .errors()
        //     .map(|e| format!("{:?}", e))
        //     .collect::<Vec<String>>()
        //     .join("\n");

        // for err in qr.errors() {
        //     println!("{:?}, {:?}, {:?}", err, err.span(), err.found());
        // }

        // let mut quotes: u32 = 0;
        // let mut slop = String::new();
        // let mut token = String::new();
        // let mut fuzziness = String::new();
        // let mut prev_char = ' ';

        // for c in query.chars() {
        //     match c {
        //         '"' => {
        //             if token.len() > 0 || fuzziness.len() > 0 {
        //                 println!("token {token}, fuzziness: {fuzziness}");
        //                 token = String::new();
        //                 fuzziness = String::new();
        //             }

        //             quotes += 1;
        //         }
        //         ' ' => {
        //             println!("token {token}, fuzziness: {fuzziness}");
        //             token = String::new();
        //             fuzziness = String::new();
        //         }
        //         '~' => {}
        //         c if c.is_digit(10) => {
        //             if (prev_char == '~' && quotes > 0 && quotes % 2 == 0)
        //                 || (quotes > 0 && quotes % 2 == 0 && slop.len() > 0)
        //             {
        //                 slop.push(c);
        //             } else if prev_char == '~' || fuzziness.len() > 0 {
        //                 fuzziness.push(c);
        //             } else {
        //                 token.push(c);
        //             }
        //         }
        //         _ => {
        //             token.push(c);
        //         }
        //     }

        //     prev_char = c;
        // }

        // if token.len() > 0 || fuzziness.len() > 0 {
        //     println!("token {token}, fuzziness: {fuzziness}");
        // }

        // println!("slop: {slop}");
    }
}
