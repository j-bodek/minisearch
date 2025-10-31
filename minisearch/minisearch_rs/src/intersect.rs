use crate::index::Posting;
use crate::tokenizer::TokenizedQuery;
use crate::trie::Trie;
use hashbrown::HashMap;
use std::cmp::{Ordering, Reverse};
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
    docs: Vec<Vec<TokenDocPointer>>,
    pointers: HashMap<String, BinaryHeap<Reverse<TokenDocPointer>>>,
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
        let mut docs: Vec<Vec<TokenDocPointer>> = vec![];
        let mut pointers: HashMap<String, BinaryHeap<Reverse<TokenDocPointer>>> = HashMap::new();

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
                    .push(Reverse(pointer));
            }

            if !pointers.contains_key(&query_token.text) {
                return None;
            }

            let (_, doc) =
                Self::next_doc_index(index, pointers.get_mut(&query_token.text).unwrap());
            docs.push(doc);
        }

        Some(Self {
            query: query,
            index: index,
            docs: docs,
            pointers: pointers,
        })
    }

    fn next_doc_index(
        index: &HashMap<String, Vec<Posting>>,
        pointer: &mut BinaryHeap<Reverse<TokenDocPointer>>,
    ) -> (f64, Vec<TokenDocPointer>) {
        let (mut max_score, mut doc_ids) = (0 as f64, Vec::<TokenDocPointer>::new());

        while !pointer.is_empty() && (doc_ids.is_empty() || doc_ids[0] == pointer.peek().unwrap().0)
        {
            let p = pointer.pop().unwrap();
            if p.0.doc_idx + 1 <= index.get(&p.0.token).unwrap().len() as u32 - 1 {
                pointer.push(Reverse(TokenDocPointer {
                    doc_id: index.get(&p.0.token).unwrap()[p.0.doc_idx as usize + 1].doc_id,
                    doc_idx: p.0.doc_idx + 1,
                    token: p.0.token.clone(),
                    distance: p.0.distance,
                }))
            }

            max_score = f64::max(
                max_score,
                index.get(&p.0.token).unwrap()[p.0.doc_idx as usize + 1].score,
            );
            doc_ids.push(p.0);
        }

        return (max_score, doc_ids);
    }
}
