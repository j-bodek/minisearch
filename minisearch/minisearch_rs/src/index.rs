use crate::trie::Trie;
use crate::utils::hasher::TokenHasher;

use std::{io, path::PathBuf};

use bincode::config::Configuration;
use bincode::de::Decoder;
use bincode::enc::write::SizeWriter;
use bincode::enc::EncoderImpl;
use bincode::error::{DecodeError, EncodeError};
use bincode::{Decode, Encode};
use hashbrown::{HashMap, HashSet};
use nohash_hasher::BuildNoHashHasher;

use ulid::Ulid;

struct LogMeta {
    id: u128,
    offset: u128,
    size: u32,
}

#[repr(u8)]
#[derive(Copy, Clone, PartialEq)]
enum LogOperation {
    DELETE = 0,
    ADD = 1,
}

trait IndexLog {
    fn encode_into_vec(&self, vec: &mut Vec<u8>) -> Result<(usize, usize), EncodeError>;
}

struct AddLog<'a> {
    token: u32,
    operation: LogOperation,
    size: u32,
    postings: &'a Vec<Posting>,
}

impl<'a> IndexLog for AddLog<'a> {
    fn encode_into_vec(&self, vec: &mut Vec<u8>) -> Result<(usize, usize), EncodeError> {
        todo!();
    }
}

impl<'a> AddLog<'a> {
    fn new(token: u32, size: u32, postings: &'a Vec<Posting>) -> Self {
        Self {
            token: token,
            operation: LogOperation::ADD,
            size: size,
            postings: postings,
        }
    }
}

struct DeleteLog {
    token: u32,
    operation: LogOperation,
    size: u32,
}

impl IndexLog for DeleteLog {
    fn encode_into_vec(&self, vec: &mut Vec<u8>) -> Result<(usize, usize), EncodeError> {
        todo!();
    }
}

impl DeleteLog {
    fn new(token: u32, size: u32) -> Self {
        Self {
            token: token,
            operation: LogOperation::DELETE,
            size: size,
        }
    }
}

#[derive(Decode, Encode, PartialEq, Debug, Clone)]
pub struct Posting {
    pub doc_id: u128,
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
            docs.retain(|doc| !document_ids.contains(&Ulid(doc.doc_id)));

            if docs.len() == 0 {
                self.index.remove(token);
                fuzzy_trie.delete(hasher.delete(*token).unwrap());
            }
        }
    }

    fn save_log<T: IndexLog>(&mut self, id: &Ulid, log: T) {
        // serialize log to bytes
        // create and serialize meta to bytes
    }
}
