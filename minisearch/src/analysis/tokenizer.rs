use std::sync::Arc;

use crate::query::parser::Query;
use crate::{analysis::stemmer::SnowballStemmer, config::Config};
use hashbrown::HashMap;
use unicode_segmentation::UnicodeSegmentation;

#[derive(Debug)]
pub struct Token {
    pub text: String,
    pub fuzz: u8,
}

pub struct TokenizedQuery {
    pub tokens: Vec<Token>,
    pub slop: u8,
}

pub struct Tokenizer {
    stemmer: SnowballStemmer,
    config: Arc<Config>,
}

impl Tokenizer {
    pub fn new(config: Arc<Config>) -> Self {
        Self {
            stemmer: SnowballStemmer::new(),
            config: config,
        }
    }

    pub fn tokenize_doc(&mut self, doc: &mut str) -> (u32, HashMap<String, Vec<u32>>) {
        let mut tokens: HashMap<String, Vec<u32>> = HashMap::new();

        let mut i = 0;
        for word in doc.unicode_words() {
            let word = word.to_owned().to_ascii_lowercase();
            if self.config.stop_words.contains(word.as_str()) {
                continue;
            }
            let word = self.stemmer.stem(word);
            tokens.entry_ref(&word).or_default().push(i);
            i += 1;
        }

        return (i, tokens);
    }

    pub fn tokenize_query(&mut self, query: Query) -> TokenizedQuery {
        let mut tokens: Vec<Token> = Vec::with_capacity(query.terms.len());

        for term in query.terms {
            if self.config.stop_words.contains(term.text) {
                continue;
            }

            let token = Token {
                text: self.stemmer.stem(term.text.to_string()),
                fuzz: term.fuzz,
            };
            tokens.push(token);
        }

        TokenizedQuery {
            tokens: tokens,
            slop: query.slop,
        }
    }
}
