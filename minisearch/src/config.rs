use serde::Deserialize;
use std::{collections::HashSet, fs, io, path::PathBuf};
use toml;

#[derive(Debug, Deserialize)]
#[serde(default)]
pub struct Config {
    // document config
    pub segment_size: u64,
    pub documents_buffer_size: u64,
    pub documents_save_after_seconds: u64,
    pub merge_deleted_ratio: f64,
    // search metadata config
    pub metadata_save_after_operations: u32,
    pub metadata_save_after_seconds: u64,
    // index config
    pub index_buffer_size: u64,
    pub index_save_after_operations: u64,
    pub index_save_after_seconds: u64,
    // additional config
    pub stop_words: HashSet<String>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            // document config
            segment_size: 1024 * 1024 * 50,
            documents_buffer_size: 1024 * 1024,
            documents_save_after_seconds: 5,
            merge_deleted_ratio: 0.3,
            // search metadata config
            metadata_save_after_operations: 100_000,
            metadata_save_after_seconds: 10,
            // index config
            index_buffer_size: 1024 * 1024,
            index_save_after_operations: 100_000,
            index_save_after_seconds: 5,
            // additional config
            stop_words: [
                "a", "and", "are", "as", "at", "be", "but", "by", "for", "if", "in", "into", "is",
                "it", "no", "not", "of", "on", "or", "s", "such", "t", "that", "the", "their",
                "then", "there", "these", "they", "this", "to", "was", "will", "with", "www",
            ]
            .map(|word| word.to_string())
            .into_iter()
            .collect(),
        }
    }
}

impl Config {
    pub fn load(path: Option<PathBuf>) -> Result<Self, io::Error> {
        let config = match path {
            Some(path) => {
                let config: String = fs::read_to_string(path)?;
                // todo handle this unwrap properly
                toml::from_str(&config).unwrap()
            }
            None => Self::default(),
        };

        Ok(config)
    }
}
