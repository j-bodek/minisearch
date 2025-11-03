use std::collections::BinaryHeap;
use std::slice::Iter;

struct TokenPositions<'a> {
    token: String,
    distance: u32,
    tfs: f64,
    positions: Iter<'a, u32>,
}

struct TokenGroupIterator<'a> {
    heap: BinaryHeap<u32>,
    tokens: Vec<TokenPositions<'a>>,
}

struct MinimalIntervalSemanticMatch<'a> {
    iterators: Vec<TokenGroupIterator<'a>>,
    window: Vec<usize>, // window of token indexes
    slops: Vec<i32>,
}
