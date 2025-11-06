use core::cmp::Ordering;
use std::collections::BinaryHeap;
use std::slice::Iter;

struct TokenPositions<'a> {
    token: String,
    distance: u32,
    tfs: f64,
    positions: Iter<'a, u32>,
}

struct TokenMeta {
    token: String,
    distance: u32,
    tfs: f64,
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

struct TokenGroupIterator<'a> {
    heap: BinaryHeap<TokenPosition>,
    tokens: Vec<TokenPositions<'a>>,
}

struct MinimalIntervalSemanticMatch<'a> {
    iterators: Vec<TokenGroupIterator<'a>>,
    window: Vec<usize>, // window of token indexes
    slops: Vec<i32>,
}

impl<'a> TokenGroupIterator<'a> {
    fn new() -> Self {
        Self {
            heap: BinaryHeap::new(),
            tokens: vec![],
        }
    }

    fn add_token_positions(
        &mut self,
        token: String,
        distance: u32,
        tfs: f64,
        mut positions: Iter<'a, u32>,
    ) {
        match positions.next() {
            Some(val) => {
                self.heap.push(TokenPosition {
                    position: *val,
                    idx: self.tokens.len(),
                });
                self.tokens.push(TokenPositions {
                    token: token,
                    distance: distance,
                    tfs: tfs,
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
                tfs: token.tfs,
            });
        }

        return None;
    }
}
