use crate::index::Posting;
use crate::tokenizer::TokenizedQuery;
use crate::trie::Trie;
use hashbrown::HashMap;
use std::cmp::{Ord, Ordering};
use std::collections::BinaryHeap;
use ulid::Ulid;

struct TokenDocPointer {
    doc_id: Ulid,
    doc_idx: u32,
    token: String,
    distance: u16,
}

pub struct PostingListIntersection<'a> {
    query: TokenizedQuery,
    index: &'a HashMap<String, Vec<Posting>>,
    docs: Vec<Ulid>,
    pointers: HashMap<String, BinaryHeap<TokenDocPointer>>,
}

impl Ord for TokenDocPointer {
    fn cmp(&self, other: &Self) -> Ordering {
        self.doc_id.cmp(&other.doc_id)
    }
}

impl PartialOrd for TokenDocPointer {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.doc_id.cmp(&other.doc_id))
    }
}

impl PartialEq for TokenDocPointer {
    fn eq(&self, other: &Self) -> bool {
        self.doc_id == other.doc_id
    }
}

impl Eq for TokenDocPointer {}

impl<'a> PostingListIntersection<'a> {
    pub fn new(
        query: TokenizedQuery,
        index: &'a HashMap<String, Vec<Posting>>,
        fuzzy_trie: &Trie,
    ) -> Option<Self> {
        let mut docs: Vec<Ulid> = vec![];
        let mut pointers: HashMap<String, BinaryHeap<TokenDocPointer>> = HashMap::new();

        for query_token in query.tokens.iter() {
            for (distance, token) in fuzzy_trie.search(query_token.fuzz, &query_token.text) {
                if query_token.text != token
                    && (token.len() <= query_token.fuzz as usize
                        || query_token.text.len() <= query_token.fuzz as usize)
                {
                    continue;
                }

                let pointer = TokenDocPointer {
                    doc_id: index.get(&token).unwrap()[0].doc_id,
                    doc_idx: 0,
                    token: token,
                    distance: distance,
                };
                pointers
                    .entry_ref(query_token.text.as_str())
                    .or_default()
                    .push(pointer);
            }

            if !pointers.contains_key(&query_token.text) {
                return None;
            }
        }

        Some(Self {
            query: query,
            index: index,
            docs: docs,
            pointers: pointers,
        })
    }
}
