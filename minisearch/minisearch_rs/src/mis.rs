use crate::index::Posting;
use crate::intersect::TokenDocPointer;
use core::cmp::Ordering;
use hashbrown::HashMap;
use std::collections::BinaryHeap;
use std::slice::Iter;
use ulid::Ulid;

struct TokenPositions<'a> {
    token: String,
    distance: u16,
    tf: u64,
    positions: Iter<'a, u32>,
}

struct TokenMeta {
    token: String,
    distance: u16,
    tf: u64,
}

struct TokenPosition {
    position: u32,
    idx: usize,
}

impl Ord for TokenPosition {
    fn cmp(&self, other: &Self) -> Ordering {
        self.position.cmp(&other.position)
    }
}

impl PartialOrd for TokenPosition {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.position.cmp(&other.position))
    }
}

impl PartialEq for TokenPosition {
    fn eq(&self, other: &Self) -> bool {
        self.position == other.position && self.idx == other.idx
    }
}

impl Eq for TokenPosition {}

#[derive(Debug)]
pub struct MisTokenIdx {
    pub token: String,
    pub token_idx: u32,
    pub tf: u64,
    pub distance: u16,
}

#[derive(Debug)]
pub struct MisResult {
    pub doc_id: Ulid,
    pub slop: i32,
    pub indexes: Vec<MisTokenIdx>,
}

struct TokenGroupIterator<'a> {
    heap: BinaryHeap<TokenPosition>,
    tokens: Vec<TokenPositions<'a>>,
}

pub struct MinimalIntervalSemanticMatch<'a> {
    doc_id: Ulid,
    min_slop: i32,
    iterators: Vec<TokenGroupIterator<'a>>,
    window: Vec<u32>, // window of token indexes
    slops: Vec<i32>,
    end: bool,
}

impl<'a> TokenGroupIterator<'a> {
    fn new() -> Self {
        Self {
            heap: BinaryHeap::new(),
            tokens: vec![],
        }
    }

    fn add_token_positions(&mut self, mut positions: Iter<'a, u32>, token: String, distance: u16) {
        match positions.next() {
            Some(val) => {
                self.heap.push(TokenPosition {
                    position: *val,
                    idx: self.tokens.len(),
                });
                self.tokens.push(TokenPositions {
                    token: token,
                    distance: distance,
                    tf: positions.len() as u64,
                    positions: positions,
                });
            }
            _ => (),
        }
    }

    fn closest(&mut self, target: u32) -> Option<u32> {
        while !self.heap.is_empty() && self.heap.peek().unwrap().position <= target {
            let pos = self.heap.pop().unwrap();
            while let Some(val) = self.tokens[pos.idx].positions.next() {
                if *val > target {
                    self.heap.push(TokenPosition {
                        position: *val,
                        idx: pos.idx,
                    });
                    break;
                }
            }
        }

        self.peek()
    }

    fn next(&mut self) -> Option<u32> {
        if !self.heap.is_empty() {
            let pos = self.heap.pop().unwrap();
            if let Some(val) = self.tokens[pos.idx].positions.next() {
                self.heap.push(TokenPosition {
                    position: *val,
                    idx: pos.idx,
                });
            }
        }

        self.peek()
    }

    fn peek(&self) -> Option<u32> {
        if !self.heap.is_empty() {
            return Some(self.heap.peek().unwrap().position);
        }

        return None;
    }

    fn last_meta(&self) -> Option<TokenMeta> {
        if !self.heap.is_empty() {
            let token = &self.tokens[self.heap.peek().unwrap().idx];
            return Some(TokenMeta {
                token: token.token.clone(),
                distance: token.distance,
                tf: token.tf,
            });
        }

        return None;
    }
}

impl<'a> MinimalIntervalSemanticMatch<'a> {
    pub fn new(
        index: &'a HashMap<String, Vec<Posting>>,
        pointers: Vec<Vec<TokenDocPointer>>,
        min_slop: i32,
    ) -> Self {
        let doc_id = pointers[0][0].doc_id;
        let mut iterators: Vec<TokenGroupIterator> = Vec::with_capacity(pointers.len());
        for group in pointers {
            let mut iterator = TokenGroupIterator::new();
            for pointer in group {
                iterator.add_token_positions(
                    index.get(&pointer.token).unwrap()[pointer.doc_idx as usize]
                        .positions
                        .iter(),
                    pointer.token,
                    pointer.distance,
                );
            }

            iterators.push(iterator);
        }

        let window = (0..iterators.len())
            .map(|i| iterators[i].peek().unwrap())
            .collect::<Vec<u32>>();

        let slops = vec![0; iterators.len()];

        Self {
            doc_id: doc_id,
            min_slop: min_slop,
            iterators: iterators,
            window: window,
            slops: slops,
            end: false,
        }
    }
}

// TODO
impl<'a> Iterator for MinimalIntervalSemanticMatch<'a> {
    type Item = MisResult;

    fn next(&mut self) -> Option<MisResult> {
        let mut idx = 1;
        while !self.end {
            while idx <= self.iterators.len() - 1 {
                let val = match self.iterators[idx].closest(self.window[idx - 1]) {
                    Some(val) => val,
                    None => return None,
                };

                self.window[idx] = val;
                let slop = self.slops[idx - 1]
                    + (self.window[idx - 1] as i32 - (self.window[idx] as i32 - 1)).abs();

                if slop > self.min_slop {
                    break;
                }

                self.slops[idx] = slop;
                idx += 1;
            }

            let mut result = None;
            if idx == self.iterators.len() {
                let mut window = vec![];
                for (iter_idx, token_idx) in self.window.iter().enumerate() {
                    let meta = self.iterators[iter_idx].last_meta().unwrap();
                    window.push((*token_idx, meta.token, meta.tf, meta.distance));
                }

                let _ = result.insert(MisResult {
                    doc_id: self.doc_id,
                    slop: self.slops[self.iterators.len() - 1],
                    indexes: window
                        .into_iter()
                        .map(|(token_idx, token, tf, distance)| MisTokenIdx {
                            token: token,
                            token_idx: token_idx,
                            tf: tf,
                            distance: distance,
                        })
                        .collect::<Vec<MisTokenIdx>>(),
                });
            }

            match self.iterators[0].next() {
                Some(val) => self.window[0] = val,
                None => self.end = true,
            };

            match result {
                Some(res) => return Some(res),
                _ => (),
            }
        }

        None
    }
}
