static K: f64 = 1.5;
static B: f64 = 0.75;
static EPS: f64 = 0.5;

pub fn term_bm25(
    tf: u64,
    docs_num: u64,
    token_docs_num: u64,
    doc_length: u64,
    avg_doc_length: f64,
) -> f64 {
    let idf =
        (((docs_num - token_docs_num) as f64 + EPS) / (token_docs_num as f64 + EPS)).ln() + 1.0;

    idf * ((tf as f64 * (K + 1.0))
        / (tf as f64 + K * (1.0 - B + B * (doc_length as f64 / avg_doc_length))))
}
