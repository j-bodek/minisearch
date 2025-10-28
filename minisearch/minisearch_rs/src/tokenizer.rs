use crate::stemmer::SnowballStemmer;
use hashbrown::HashMap;
use unicode_segmentation::UnicodeSegmentation;

static STOP_WORDS: [&str; 35] = [
    "a", "and", "are", "as", "at", "be", "but", "by", "for", "if", "in", "into", "is", "it", "no",
    "not", "of", "on", "or", "s", "such", "t", "that", "the", "their", "then", "there", "these",
    "they", "this", "to", "was", "will", "with", "www",
];

pub struct Tokenizer {
    stemmer: SnowballStemmer,
}

impl Tokenizer {
    pub fn new() -> Self {
        Self {
            stemmer: SnowballStemmer::new(),
        }
    }

    pub fn tokenize_doc(&mut self, doc: String) -> (u32, HashMap<String, Vec<u32>>) {
        let mut tokens: HashMap<String, Vec<u32>> = HashMap::new();

        let mut i = 0;
        for word in doc.unicode_words() {
            if STOP_WORDS.contains(&word) {
                continue;
            }
            let word = self.stemmer.stem(word.to_string());
            tokens.entry_ref(&word).or_default().push(i);
            i += 1;
        }

        return (i, tokens);
    }
}
