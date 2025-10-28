use crate::index::Posting;
use crate::tokenizer::TokenizedQuery;
use crate::trie::Trie;
use hashbrown::HashMap;

pub struct PostingListIntersection<'a> {
    query: TokenizedQuery,
    index: &'a HashMap<String, Vec<Posting>>,
}

impl<'a> PostingListIntersection<'a> {
    pub fn new(
        query: TokenizedQuery,
        index: &'a HashMap<String, Vec<Posting>>,
        fuzzy_trie: &Trie,
    ) -> Self {
        Self {
            query: query,
            index: index,
        }
    }
}
