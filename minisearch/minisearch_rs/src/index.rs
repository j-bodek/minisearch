use crate::trie::Trie;
use crate::utils::hasher::TokenHasher;

use std::array::TryFromSliceError;
use std::borrow::Cow;
use std::error::Error;
use std::fs::File;
use std::hash::Hash;
use std::io::{Read, Seek, Write};
use std::marker::PhantomData;
use std::os::unix::fs::FileExt;
use std::{io, path::PathBuf};

use bincode::config::Configuration;
use bincode::enc::write::SizeWriter;
use bincode::enc::EncoderImpl;
use bincode::error::EncodeError;
use bincode::{Decode, Encode};
use chumsky::prelude::empty;
use hashbrown::hash_map::Entry;
use hashbrown::{HashMap, HashSet};
use nohash_hasher::BuildNoHashHasher;
use std::fmt::Debug;
use thiserror::Error;

use ulid::Ulid;

static BUFFER_THRESHOLD: u64 = 1024 * 1024;

#[derive(Debug)]
struct LogMeta {
    id: u128,
    offset: u64,
    size: u32,
}

impl LogMeta {
    const ENCODED_SIZE: usize = 28;

    fn from_bytes(bytes: [u8; Self::ENCODED_SIZE]) -> Result<Self, TryFromSliceError> {
        Ok(Self {
            id: u128::from_be_bytes(bytes[..16].try_into()?),
            offset: u64::from_be_bytes(bytes[16..24].try_into()?),
            size: u32::from_be_bytes(bytes[24..].try_into()?),
        })
    }

    fn encode_into_vec(&self, vec: &mut Vec<u8>) {
        let (size, offset) = (Self::ENCODED_SIZE, vec.len());

        vec.resize(offset + size, 0);
        vec[offset..offset + 16].copy_from_slice(&self.id.to_be_bytes());
        vec[offset + 16..offset + 24].copy_from_slice(&self.offset.to_be_bytes());
        vec[offset + 24..offset + 28].copy_from_slice(&self.size.to_be_bytes());
    }
}

#[repr(u8)]
#[derive(Copy, Clone, PartialEq, Debug)]
enum LogOperation {
    DELETE = 0,
    ADD = 1,
}

// TODO: create proper errors later
#[derive(Error, Debug)]
enum FromBytesError {
    #[error("Failed to convert data from slice")]
    TryFromSliceError(#[from] TryFromSliceError),
    #[error("Failed to decode data")]
    DecodeError,
}

impl LogOperation {
    fn from_u8(val: u8) -> Result<Self, FromBytesError> {
        match val {
            0 => Ok(Self::DELETE),
            1 => Ok(Self::ADD),
            _ => Err(FromBytesError::DecodeError),
        }
    }
}

trait IndexLog: Debug {
    fn encode_into_vec(&self, vec: &mut Vec<u8>) -> Result<(usize, usize), EncodeError>;
    fn from_bytes(bytes: &mut [u8]) -> Self;
}

enum IndexLogImpl<'a> {
    Add(AddLog<'a>),
    Delete(DeleteLog),
}

impl<'a> IndexLogImpl<'a> {
    fn header(&self) -> &LogHeader {
        match self {
            Self::Add(log) => &log.header,
            Self::Delete(log) => &log.header,
        }
    }
}

fn decode_log<'a>(bytes: &mut [u8]) -> Result<IndexLogImpl<'a>, FromBytesError> {
    let operation = LogOperation::from_u8(u8::from_be_bytes(bytes[..1].try_into()?))?;
    match operation {
        LogOperation::ADD => Ok(IndexLogImpl::Add(AddLog::from_bytes(bytes))),
        LogOperation::DELETE => Ok(IndexLogImpl::Delete(DeleteLog::from_bytes(bytes))),
    }
}

#[derive(Debug)]
struct LogHeader {
    operation: LogOperation,
    token: u32,
    postings_num: u32,
}

impl LogHeader {
    const ENCODED_SIZE: usize = 9;

    fn from_bytes(bytes: [u8; Self::ENCODED_SIZE]) -> Result<Self, FromBytesError> {
        Ok(Self {
            operation: LogOperation::from_u8(u8::from_be_bytes(bytes[..1].try_into()?))?,
            token: u32::from_be_bytes(bytes[1..5].try_into()?),
            postings_num: u32::from_be_bytes(bytes[5..].try_into()?),
        })
    }

    fn encode_into_vec(&self, vec: &mut Vec<u8>) -> usize {
        let offset = vec.len();

        vec.resize(offset + Self::ENCODED_SIZE, 0);
        vec[offset..offset + 1].copy_from_slice(&(self.operation as u8).to_be_bytes());
        vec[offset + 1..offset + 5].copy_from_slice(&self.token.to_be_bytes());
        vec[offset + 5..offset + 9].copy_from_slice(&self.postings_num.to_be_bytes());

        Self::ENCODED_SIZE
    }
}

#[derive(Debug)]
struct AddLog<'a> {
    header: LogHeader,
    posting: Cow<'a, Posting>,
}

impl<'a> IndexLog for AddLog<'a> {
    fn from_bytes(bytes: &mut [u8]) -> Self {
        let header =
            LogHeader::from_bytes(bytes[..LogHeader::ENCODED_SIZE].try_into().unwrap()).unwrap();
        let (posting, _): (Posting, usize) = bincode::decode_from_slice(
            &bytes[LogHeader::ENCODED_SIZE..],
            bincode::config::standard(),
        )
        .unwrap();

        Self {
            header: header,
            posting: Cow::Owned(posting),
        }
    }

    fn encode_into_vec(&self, vec: &mut Vec<u8>) -> Result<(usize, usize), EncodeError> {
        let offset = vec.len();

        let header_size = self.header.encode_into_vec(vec);

        let config = bincode::config::standard();
        let posting_size = {
            let mut size_writer =
                EncoderImpl::<_, Configuration>::new(SizeWriter::default(), config);
            self.posting.encode(&mut size_writer)?;
            size_writer.into_writer().bytes_written
        };

        vec.resize(offset + header_size + posting_size, 0);
        let posting_size =
            bincode::encode_into_slice(&self.posting, &mut vec[offset + header_size..], config)
                .unwrap();

        vec.truncate(offset + header_size + posting_size);

        // return encode result (offset, size)
        Ok((offset, header_size + posting_size))
    }
}

impl<'a> AddLog<'a> {
    fn new(token: u32, postings_num: u32, posting: &'a Posting) -> Self {
        Self {
            header: LogHeader {
                token: token,
                operation: LogOperation::ADD,
                postings_num: postings_num,
            },
            posting: Cow::Borrowed(posting),
        }
    }
}

#[derive(Debug)]
struct DeleteLog {
    header: LogHeader,
}

impl IndexLog for DeleteLog {
    fn from_bytes(bytes: &mut [u8]) -> Self {
        // TODO: proper error handling
        Self {
            header: LogHeader::from_bytes(bytes.try_into().unwrap()).unwrap(),
        }
    }

    fn encode_into_vec(&self, vec: &mut Vec<u8>) -> Result<(usize, usize), EncodeError> {
        let offset = vec.len();
        let header_size = self.header.encode_into_vec(vec);
        Ok((offset, header_size))
    }
}

impl DeleteLog {
    fn new(token: u32, postings_num: u32) -> Self {
        Self {
            header: LogHeader {
                token: token,
                operation: LogOperation::DELETE,
                postings_num: postings_num,
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
                let index = File::open(&self.dir.join("index"))?;
                self.index_size.replace(index.metadata()?.len());
                Ok(self.index_size.unwrap())
            }
        }
    }

    fn write<T: IndexLog>(&mut self, doc_id: u128, log: T) -> Result<(), Box<dyn Error>> {
        let (offset, size) = log.encode_into_vec(&mut self.index)?;

        let meta = LogMeta {
            id: doc_id,
            offset: self.get_index_size()? + offset as u64,
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

enum ReadDirection {
    FORWARD,
    BACKWARD,
}

struct MetaReader {
    file: File,
    file_size: u64,
    offset: i64,
    direction: ReadDirection,
}

impl MetaReader {
    fn new(file_path: PathBuf, direction: ReadDirection) -> Result<Self, io::Error> {
        let file = File::open(file_path).unwrap();
        let file_size = file.metadata().unwrap().len();
        let offset = if let ReadDirection::FORWARD = direction {
            0
        } else {
            file_size as i64 - LogMeta::ENCODED_SIZE as i64
        };

        Ok(Self {
            file: file,
            file_size: file_size,
            offset: offset,
            direction: direction,
        })
    }
}

impl Iterator for MetaReader {
    type Item = Result<LogMeta, io::Error>;

    fn next(&mut self) -> Option<Self::Item> {
        let mut buf = [0u8; LogMeta::ENCODED_SIZE];
        match self.direction {
            ReadDirection::FORWARD => {
                if self.file.stream_position().unwrap() >= self.file_size {
                    return None;
                }
            }
            ReadDirection::BACKWARD => {
                if self.offset < 0 {
                    return None;
                }
                if let Err(e) = self.file.seek(io::SeekFrom::Start(self.offset as u64)) {
                    return Some(Err(e));
                };
                self.offset -= LogMeta::ENCODED_SIZE as i64;
            }
        }

        if let Err(e) = self.file.read_exact(&mut buf) {
            return Some(Err(e));
        };

        Some(Ok(LogMeta::from_bytes(buf).unwrap()))
    }
}

struct LogsReader<'a> {
    _marker: PhantomData<&'a ()>,
    file: File,
    meta_reader: MetaReader,
}

impl<'a> LogsReader<'a> {
    fn new(index_dir: &PathBuf, direction: ReadDirection) -> Result<Self, io::Error> {
        Ok(Self {
            _marker: PhantomData,
            file: File::open(index_dir.join("index"))?,
            meta_reader: MetaReader::new(index_dir.join("meta"), direction)?,
        })
    }
}

impl<'a> Iterator for LogsReader<'a> {
    type Item = Result<(LogMeta, IndexLogImpl<'a>), io::Error>;

    fn next(&mut self) -> Option<Self::Item> {
        let meta = match self.meta_reader.next() {
            Some(m) => match m {
                Ok(log) => log,
                Err(e) => return Some(Err(e)),
            },
            None => return None,
        };

        let mut buf = vec![0u8; meta.size as usize];
        if let Err(e) = self.file.read_exact_at(&mut buf, meta.offset) {
            return Some(Err(e));
        };

        // TODO: add proper error handling
        let log = decode_log(&mut buf).unwrap();

        Some(Ok((meta, log)))
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

    fn load(
        &self,
        direction: ReadDirection,
    ) -> Result<HashMap<u32, Vec<Posting>, BuildNoHashHasher<u32>>, io::Error> {
        let reader = LogsReader::new(&self.buffer.dir, direction)?;

        let mut index: HashMap<u32, Vec<Posting>, BuildNoHashHasher<u32>> = HashMap::default();
        let mut tokens_cur_index: HashMap<u32, usize, BuildNoHashHasher<u32>> = HashMap::default();
        let mut deleted: HashSet<u128> = HashSet::default();
        let mut empty_postings = vec![];

        for res in reader {
            let (meta, log) = res?;

            let idx = tokens_cur_index
                .entry(log.header().token)
                .or_insert(log.header().postings_num as usize - 1);

            let postings = match index.entry(log.header().token) {
                Entry::Vacant(e) => {
                    if log.header().postings_num == 0 {
                        empty_postings.push(log.header().token)
                    }
                    e.insert(vec![Posting::default(); log.header().postings_num as usize])
                }
                Entry::Occupied(e) => e.into_mut(),
            };

            match log {
                IndexLogImpl::Add(log) => {
                    if !deleted.contains(&meta.id) {
                        postings[*idx] = log.posting.into_owned();
                        *idx -= 1;
                    }
                }
                IndexLogImpl::Delete(_) => {
                    deleted.insert(meta.id);
                }
            }
        }

        for token in empty_postings {
            index.remove(&token);
        }

        Ok(index)
    }

    fn flush(&mut self) -> Result<(), io::Error> {
        self.buffer.flush()
    }
}

#[derive(Decode, Encode, PartialEq, Debug, Clone)]
pub struct Posting {
    pub doc_id: u128,
    pub positions: Vec<u32>,
    pub score: f64,
}

impl Posting {
    fn default() -> Self {
        Self {
            doc_id: 0,
            positions: Vec::with_capacity(0),
            score: 0.0,
        }
    }
}

pub struct IndexManager {
    logs_manager: LogsManager,
    pub index: HashMap<u32, Vec<Posting>, BuildNoHashHasher<u32>>,
}

impl IndexManager {
    pub fn load(dir: &PathBuf) -> Result<Self, io::Error> {
        // TODO: create file structure if not exists
        let logs_manager = LogsManager::new(dir.join("index"));

        Ok(Self {
            index: logs_manager.load(ReadDirection::BACKWARD)?,
            logs_manager: logs_manager,
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
    ) -> Result<(), Box<dyn Error>> {
        for token in tokens {
            let postings = match self.index.get_mut(token) {
                Some(postings) => postings,
                _ => continue,
            };

            let (len, mut deleted) = (postings.len(), 0);
            let mut error = None;

            postings.retain(|doc| {
                if document_ids.contains(&Ulid(doc.doc_id)) {
                    deleted += 1;
                    if let Err(err) = self
                        .logs_manager
                        .write(doc.doc_id, DeleteLog::new(*token, (len - deleted) as u32))
                    {
                        error.replace(err);
                    };
                    return false;
                }

                true
            });

            if let Some(err) = error {
                return Err(err);
            }

            if postings.len() == 0 {
                self.index.remove(token);
                if let Some(token) = hasher.delete(*token)? {
                    fuzzy_trie.delete(token);
                }
            }
        }

        Ok(())
    }

    pub fn flush(&mut self) -> Result<(), io::Error> {
        self.logs_manager.flush()
    }
}
