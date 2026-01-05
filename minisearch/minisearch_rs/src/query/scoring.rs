use crate::core::index::Posting;
use crate::matching::intersect::TokenDocPointer;
use crate::matching::mis::MisResult;
use crate::storage::documents::DocumentsManager;
use hashbrown::HashMap;
use nohash_hasher::BuildNoHashHasher;

static K: f64 = 1.5;
static B: f64 = 0.75;
static EPS: f64 = 0.5;
static FUZZINESS_PENALTY: f64 = 0.8;

pub fn term_bm25(
    tf: u64,
    docs_num: u64,
    token_docs_num: u64,
    doc_length: u32,
    avg_doc_length: f64,
    distance: u16,
) -> f64 {
    let idf =
        (((docs_num - token_docs_num) as f64 + EPS) / (token_docs_num as f64 + EPS) + 1.0).ln();

    let bm25 = idf
        * ((tf as f64 * (K + 1.0))
            / (tf as f64 + K * (1.0 - B + B * (doc_length as f64 / avg_doc_length))));

    bm25 * FUZZINESS_PENALTY.powi(distance as i32)
}

pub fn bm25(
    docs_num: u64,
    doc_length: u32,
    avg_doc_length: f64,
    index: &HashMap<u32, Vec<Posting>, BuildNoHashHasher<u32>>,
    mis_result: MisResult,
) -> f64 {
    let mut score = 0.0;
    for mis_idx in mis_result.indexes {
        score += term_bm25(
            mis_idx.tf,
            docs_num,
            index.get(&mis_idx.token).unwrap_or(&vec![]).len() as u64,
            doc_length,
            avg_doc_length,
            mis_idx.distance,
        );
    }

    score / (mis_result.slop + 1) as f64
}

pub fn max_bm25(
    docs_manager: &DocumentsManager,
    avg_doc_length: f64,
    pointers: &Vec<Vec<TokenDocPointer>>,
) -> f64 {
    let mut score: f64 = 0.0;
    let docs_num = docs_manager.docs.len() as u64;
    let doc_length = docs_manager
        .docs
        .get(&pointers[0][0].doc_id)
        .unwrap()
        .tokens
        .len() as u32;

    for pointer in pointers {
        let mut max: f64 = 0.0;
        for token_doc_pointer in pointer {
            max = max.max(term_bm25(
                token_doc_pointer.tf,
                docs_num,
                token_doc_pointer.postings_len,
                doc_length,
                avg_doc_length,
                token_doc_pointer.distance,
            ));
        }
        score += max;
    }

    score
}
