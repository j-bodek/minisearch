use crate::index::Posting;
use hashbrown::HashMap;

use crate::mis::MisResult;

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
) -> f64 {
    let idf =
        (((docs_num - token_docs_num) as f64 + EPS) / (token_docs_num as f64 + EPS) + 1.0).ln();

    idf * ((tf as f64 * (K + 1.0))
        / (tf as f64 + K * (1.0 - B + B * (doc_length as f64 / avg_doc_length))))
}

pub fn bm25(
    docs_num: u64,
    doc_length: u32,
    avg_doc_length: f64,
    index: &HashMap<u32, Vec<Posting>>,
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
        ) * FUZZINESS_PENALTY.powi(mis_idx.distance as i32);
    }

    score / (mis_result.slop + 1) as f64
}
