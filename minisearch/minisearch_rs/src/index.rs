use crate::trie::Trie;
use crate::utils::hasher::TokenHasher;

use std::error::Error;
use std::fs::File;
use std::io::Write;
use std::{io, path::PathBuf};

use bincode::config::Configuration;
use bincode::enc::write::SizeWriter;
use bincode::enc::EncoderImpl;
use bincode::error::EncodeError;
use bincode::{Decode, Encode};
use hashbrown::{HashMap, HashSet};
use nohash_hasher::BuildNoHashHasher;

use ulid::Ulid;

static BUFFER_THRESHOLD: u64 = 1024 * 1024;

struct LogMeta {
    id: u128,
    offset: u64,
    size: u32,
}

impl LogMeta {
    fn encode_into_vec(&self, vec: &mut Vec<u8>) {
        let (size, offset) = (28, vec.len());

        vec.resize(offset + size, 0);
        vec[offset..offset + 16].copy_from_slice(&self.id.to_be_bytes());
        vec[offset + 16..offset + 24].copy_from_slice(&self.offset.to_be_bytes());
        vec[offset + 24..offset + 28].copy_from_slice(&self.size.to_be_bytes());
    }
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

struct LogHeader {
    token: u32,
    operation: LogOperation,
    size: u32,
}

impl LogHeader {
    fn encode_into_vec(&self, vec: &mut Vec<u8>) -> usize {
        let head_size = 9;
        let offset = vec.len();

        vec.resize(offset + head_size, 0);
        vec[offset..offset + 4].copy_from_slice(&self.token.to_be_bytes());
        vec[offset + 4..offset + 5].copy_from_slice(&(self.operation as u8).to_be_bytes());
        vec[offset + 5..offset + 9].copy_from_slice(&self.size.to_be_bytes());

        head_size
    }
}

struct AddLog<'a> {
    header: LogHeader,
    postings: &'a Posting,
}

impl<'a> IndexLog for AddLog<'a> {
    fn encode_into_vec(&self, vec: &mut Vec<u8>) -> Result<(usize, usize), EncodeError> {
        let offset = vec.len();

        let header_size = self.header.encode_into_vec(vec);

        let config = bincode::config::standard();
        let posting_size = {
            let mut size_writer =
                EncoderImpl::<_, Configuration>::new(SizeWriter::default(), config);
            self.postings.encode(&mut size_writer)?;
            size_writer.into_writer().bytes_written
        };

        vec.resize(offset + header_size + posting_size, 0);
        let posting_size =
            bincode::encode_into_slice(self.postings, &mut vec[offset + header_size..], config)
                .unwrap();

        vec.truncate(offset + header_size + posting_size);

        // return encode result (offset, size)
        Ok((offset, header_size + posting_size))
    }
}

impl<'a> AddLog<'a> {
    fn new(token: u32, size: u32, postings: &'a Posting) -> Self {
        Self {
            header: LogHeader {
                token: token,
                operation: LogOperation::ADD,
                size: size,
            },
            postings: postings,
        }
    }
}

struct DeleteLog {
    header: LogHeader,
}

impl IndexLog for DeleteLog {
    fn encode_into_vec(&self, vec: &mut Vec<u8>) -> Result<(usize, usize), EncodeError> {
        let offset = vec.len();
        let header_size = self.header.encode_into_vec(vec);
        Ok((offset, header_size))
    }
}

impl DeleteLog {
    fn new(token: u32, size: u32) -> Self {
        Self {
            header: LogHeader {
                token: token,
                operation: LogOperation::DELETE,
                size: size,
            },
        }
    }
}

struct Buffer {
    dir: PathBuf,
    index_size: Option<u64>,
    index: Vec<u8>,
    meta: Vec<u8>,
}

impl Buffer {
    fn get_index_size(&mut self) -> Result<u64, io::Error> {
        match self.index_size {
            Some(size) => Ok(size),
            None => {
                let index = File::options().open(&self.dir.join("index"))?;
                self.index_size.replace(index.metadata()?.len());
                Ok(self.index_size.unwrap())
            }
        }
    }

    fn write<T: IndexLog>(&mut self, doc_id: u128, log: T) -> Result<(), Box<dyn Error>> {
        let (offset, size) = log.encode_into_vec(&mut self.index)?;

        let meta = LogMeta {
            id: doc_id,
            offset: self.get_index_size().unwrap() + offset as u64,
            size: size as u32,
        };
        meta.encode_into_vec(&mut self.meta);

        Ok(())
    }

    fn flush(&mut self) -> Result<(), io::Error> {
        let mut index = File::options().append(true).open(&self.dir.join("index"))?;
        index.write_all(&self.index)?;

        let mut meta = File::options().append(true).open(&self.dir.join("meta"))?;
        meta.write_all(&self.meta)?;

        self.index.clear();
        self.meta.clear();
        self.index_size.take();

        Ok(())
    }
}

struct LogsManager {
    buffer: Buffer,
}

impl LogsManager {
    fn new(dir: PathBuf) -> Self {
        Self {
            buffer: Buffer {
                dir: dir,
                index_size: None,
                index: Vec::new(),
                meta: Vec::new(),
            },
        }
    }

    fn write<T: IndexLog>(&mut self, doc_id: u128, log: T) -> Result<(), Box<dyn Error>> {
        self.buffer.write(doc_id, log)?;

        if self.buffer.index.len() as u64 > BUFFER_THRESHOLD {
            self.buffer.flush()?;
        }

        Ok(())
    }
}

#[derive(Decode, Encode, PartialEq, Debug, Clone)]
pub struct Posting {
    pub doc_id: u128,
    pub positions: Vec<u32>,
    pub score: f64,
}

pub struct IndexManager {
    logs_manager: LogsManager,
    pub index: HashMap<u32, Vec<Posting>, BuildNoHashHasher<u32>>,
}

impl IndexManager {
    pub fn load(dir: &PathBuf) -> Result<Self, io::Error> {
        // TODO: implement load functionality
        Ok(Self {
            logs_manager: LogsManager::new(dir.join("index")),
            index: HashMap::default(),
        })
    }

    pub fn insert(&mut self, token: u32, posting: Posting) -> Result<(), Box<dyn Error>> {
        let postings = self.index.entry(token).or_default();
        let log = AddLog::new(token, postings.len() as u32 + 1, &posting);
        self.logs_manager.write(posting.doc_id, log)?;

        postings.push(posting);
        Ok(())
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
}
