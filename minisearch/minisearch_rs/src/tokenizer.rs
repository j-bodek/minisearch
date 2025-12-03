use crate::parser::Query;
use crate::stemmer::SnowballStemmer;
use hashbrown::{HashMap, HashSet};
use unicode_segmentation::UnicodeSegmentation;

static STOP_WORDS: [&str; 35] = [
    "a", "and", "are", "as", "at", "be", "but", "by", "for", "if", "in", "into", "is", "it", "no",
    "not", "of", "on", "or", "s", "such", "t", "that", "the", "their", "then", "there", "these",
    "they", "this", "to", "was", "will", "with", "www",
];

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
}

impl Tokenizer {
    pub fn new() -> Self {
        Self {
            stemmer: SnowballStemmer::new(),
        }
    }

    pub fn docs_tokens(&self, docs: Vec<String>) -> HashSet<String> {
        let mut tokens: HashSet<String> = HashSet::new();

        for doc in docs {
            for word in doc.unicode_words() {
                if STOP_WORDS.contains(&word) {
                    continue;
                }

                if tokens.contains(word) {
                    continue;
                }

                tokens.insert(word.to_ascii_lowercase());
            }
        }

        return tokens;
    }

    pub fn tokenize_doc(&mut self, doc: &mut str) -> (u32, HashMap<String, Vec<u32>>) {
        let mut tokens: HashMap<String, Vec<u32>> = HashMap::new();

        let mut i = 0;
        for word in doc.unicode_words() {
            let word = word.to_owned().to_ascii_lowercase();
            if STOP_WORDS.contains(&word.as_str()) {
                continue;
            }
            let word = self.stemmer.stem(word);
            tokens.entry_ref(&word).or_default().push(i);
            i += 1;
        }

        return (i, tokens);
    }

    pub fn tokenize_query(&mut self, query: Query) -> TokenizedQuery {
        // TODO: unicode segmentation for query?
        let mut tokens: Vec<Token> = Vec::with_capacity(query.terms.len());

        for term in query.terms {
            if STOP_WORDS.contains(&term.text) {
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
