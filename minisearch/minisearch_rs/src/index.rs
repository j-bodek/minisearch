use crate::trie::Trie;
use crate::utils::hasher::TokenHasher;

use std::{io, path::PathBuf};

use hashbrown::{HashMap, HashSet};
use nohash_hasher::BuildNoHashHasher;

use ulid::Ulid;

pub struct Posting {
    pub doc_id: Ulid,
    pub positions: Vec<u32>,
    pub score: f64,
}

pub struct IndexManager {
    pub index: HashMap<u32, Vec<Posting>, BuildNoHashHasher<u32>>,
}

impl IndexManager {
    pub fn load(dir: &PathBuf) -> Result<Self, io::Error> {
        // TODO: implement load functionality
        Ok(Self {
            index: HashMap::default(),
        })
    }

    pub fn insert(&mut self, token: u32, posting: Posting) {
        // TODO: add to log file
        self.index.entry(token).or_default().push(posting);
    }

    pub fn delete(
        &mut self,
        tokens: &HashSet<u32>,
        document_ids: &HashSet<Ulid>,
        fuzzy_trie: &mut Trie,
        hasher: &mut TokenHasher,
    ) {
        // TODO: add serialization and write to file logic
        for token in tokens {
            let docs = match self.index.get_mut(token) {
                Some(docs) => docs,
                _ => continue,
            };

            // TODO: write filtered out postings as logs
            docs.retain(|doc| !document_ids.contains(&doc.doc_id));

            if docs.len() == 0 {
                self.index.remove(token);
                fuzzy_trie.delete(hasher.delete(*token).unwrap());
            }
        }
    }
}
