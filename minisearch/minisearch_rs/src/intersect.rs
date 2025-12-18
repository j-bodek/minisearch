use crate::index::Posting;
use crate::tokenizer::TokenizedQuery;
use crate::trie::Trie;
use crate::utils::hasher::TokenHasher;
use hashbrown::HashMap;
use nohash_hasher::BuildNoHashHasher;
use std::cmp::{max, Ordering, Reverse};
use std::collections::BinaryHeap;
use ulid::Ulid;

#[derive(Clone, Debug)]
pub struct TokenDocPointer {
    pub doc_id: Ulid,
    pub doc_idx: u32,
    pub token: u32,
    pub distance: u16,
}

pub struct PostingListIntersection<'a> {
    query: TokenizedQuery,
    index: &'a HashMap<u32, Vec<Posting>, BuildNoHashHasher<u32>>,
    docs: Vec<Vec<TokenDocPointer>>,
    pointers: Vec<BinaryHeap<Reverse<TokenDocPointer>>>,
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
        index: &'a HashMap<u32, Vec<Posting>, BuildNoHashHasher<u32>>,
        hasher: &TokenHasher,
        fuzzy_trie: &Trie,
    ) -> Option<Self> {
        let docs: Vec<Vec<TokenDocPointer>> = Vec::with_capacity(query.tokens.len());
        let mut pointers: Vec<BinaryHeap<Reverse<TokenDocPointer>>> =
            vec![BinaryHeap::new(); query.tokens.len()];

        for (i, query_token) in query.tokens.iter().enumerate() {
            for (distance, token) in fuzzy_trie.search(query_token.fuzz, &query_token.text) {
                if query_token.text != token
                    && (token.len() <= query_token.fuzz as usize
                        || query_token.text.len() <= query_token.fuzz as usize)
                {
                    continue;
                }

                let token = match hasher.hash(&token) {
                    Some(val) => val,
                    _ => continue,
                };

                let postings = match index.get(&token) {
                    Some(val) => val,
                    _ => continue,
                };

                let pointer = TokenDocPointer {
                    doc_id: Ulid(postings[0].doc_id),
                    doc_idx: 0,
                    token: token,
                    distance: distance,
                };
                pointers[i].push(Reverse(pointer));
            }

            if pointers[i].is_empty() {
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

    fn next_docs(
        index: &HashMap<u32, Vec<Posting>, BuildNoHashHasher<u32>>,
        pointer: &mut BinaryHeap<Reverse<TokenDocPointer>>,
    ) -> (f64, Vec<TokenDocPointer>) {
        let (mut max_score, mut doc_ids) = (0 as f64, Vec::<TokenDocPointer>::new());

        while !pointer.is_empty() && (doc_ids.is_empty() || doc_ids[0] == pointer.peek().unwrap().0)
        {
            let p = pointer.pop().unwrap();
            if p.0.doc_idx + 1 <= index.get(&p.0.token).unwrap().len() as u32 - 1 {
                pointer.push(Reverse(TokenDocPointer {
                    doc_id: Ulid(index.get(&p.0.token).unwrap()[p.0.doc_idx as usize + 1].doc_id),
                    doc_idx: p.0.doc_idx + 1,
                    token: p.0.token.clone(),
                    distance: p.0.distance,
                }))
            }

            max_score = f64::max(
                max_score,
                index.get(&p.0.token).unwrap()[p.0.doc_idx as usize].score,
            );
            doc_ids.push(p.0);
        }

        return (max_score, doc_ids);
    }

    fn geq_docs(
        index: &HashMap<u32, Vec<Posting>, BuildNoHashHasher<u32>>,
        pointer: &mut BinaryHeap<Reverse<TokenDocPointer>>,
        target_doc: &Ulid,
    ) -> (f64, Vec<TokenDocPointer>) {
        while !pointer.is_empty() && pointer.peek().unwrap().0.doc_id < *target_doc {
            let doc = pointer.pop().unwrap();
            let new_idx = match index
                .get(&doc.0.token)
                .unwrap()
                .binary_search_by(|posting| posting.doc_id.cmp(&target_doc.0))
            {
                Ok(idx) => idx,
                Err(idx) => idx,
            };

            if new_idx <= index.get(&doc.0.token).unwrap().len() - 1 {
                pointer.push(Reverse(TokenDocPointer {
                    doc_id: Ulid(index.get(&doc.0.token).unwrap()[new_idx].doc_id),
                    doc_idx: new_idx as u32,
                    token: doc.0.token.clone(),
                    distance: doc.0.distance,
                }))
            }
        }

        return Self::next_docs(index, pointer);
    }
}

impl<'a> Iterator for PostingListIntersection<'a> {
    type Item = Vec<Vec<TokenDocPointer>>;
    fn next(&mut self) -> Option<Vec<Vec<TokenDocPointer>>> {
        let mut same = true;

        for i in 0..self.query.tokens.len() {
            let (_, docs) = Self::next_docs(self.index, &mut self.pointers[i]);

            if docs.is_empty() {
                return None;
            }

            if self.docs.len() <= i {
                self.docs.push(docs);
            } else {
                self.docs[i] = docs;
            }

            if i != 0 && self.docs[i][0].doc_id != self.docs[i - 1][0].doc_id {
                same = false;
            }
        }

        let mut target_doc = self.docs.iter().max_by(|x, y| x[0].cmp(&y[0])).unwrap()[0].doc_id;
        loop {
            if same {
                return Some(self.docs.clone());
            } else {
                same = true;
                let cur_target_doc = target_doc.clone();
                for i in 0..self.query.tokens.len() {
                    if cur_target_doc != self.docs[i][0].doc_id {
                        let (_, docs) =
                            Self::geq_docs(self.index, &mut self.pointers[i], &target_doc);

                        if docs.is_empty() {
                            return None;
                        }

                        target_doc = max(target_doc, docs[0].doc_id);
                        self.docs[i] = docs;
                    }

                    if i != 0 && self.docs[i][0].doc_id != self.docs[i - 1][0].doc_id {
                        same = false;
                    }
                }
            }
        }
    }
}
