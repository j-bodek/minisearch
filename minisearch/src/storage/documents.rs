use bincode::config::Configuration;
use bincode::enc::EncoderImpl;
use bincode::enc::write::SizeWriter;
use bincode::error::{DecodeError, EncodeError};
use bincode::{Decode, Encode};
use hashbrown::{HashMap, HashSet};
use lz4_flex::block::{
    CompressError, compress_into, decompress_size_prepended, get_maximum_output_size,
};
use pyo3::exceptions::{PySystemError, PyValueError};
use pyo3::prelude::*;
use std::fs::remove_dir_all;
use std::io::{self, prelude::*};
use std::os::unix::prelude::FileExt;
use std::sync::Arc;
use std::time::SystemTimeError;
use std::{
    fs::{self, File},
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};
use thiserror::Error;
use ulid::Ulid;

use crate::config::Config;
use crate::errors::{BincodeDecodeError, BincodeEncodeError, CompressException};

#[derive(Error, Debug)]
pub enum DocumentBufferError {
    #[error("documents buffer: compress failed: {0}")]
    CompressError(#[from] CompressError),
    #[error("documents buffer: bincode encode failed: {0}")]
    BincodeEncodeError(#[from] EncodeError),
}

impl From<DocumentBufferError> for pyo3::PyErr {
    fn from(err: DocumentBufferError) -> Self {
        match err {
            DocumentBufferError::CompressError(err) => CompressException::new_err(err.to_string()),
            DocumentBufferError::BincodeEncodeError(err) => {
                BincodeEncodeError::new_err(err.to_string())
            }
        }
    }
}

#[derive(Error, Debug)]
pub enum DocumentsManagerError {
    #[error("documents manager: io error: {0}")]
    Io(#[from] io::Error),
    #[error("documents manager: system time error: {0}")]
    Time(#[from] SystemTimeError),
    #[error("documents manager: bincode decode failed: {0}")]
    BincodeDecodeError(#[from] DecodeError),
    #[error("documents manager: document buffer error: {0}")]
    DocumentBufferError(#[from] DocumentBufferError),
}

impl From<DocumentsManagerError> for pyo3::PyErr {
    fn from(err: DocumentsManagerError) -> Self {
        match err {
            DocumentsManagerError::Io(err) => err.into(),
            DocumentsManagerError::Time(err) => PySystemError::new_err(err.to_string()),
            DocumentsManagerError::BincodeDecodeError(err) => {
                BincodeDecodeError::new_err(err.to_string())
            }
            DocumentsManagerError::DocumentBufferError(err) => err.into(),
        }
    }
}

#[pyclass(name = "Document")]
#[derive(Decode, Encode, PartialEq, Debug, Clone)]
pub struct Document {
    pub id: [u8; 16], // binary representation of ULID
    data: Option<String>,
    pub location: DocLocation,
    pub len: u32,
    pub tokens: Vec<u32>,
}

impl Document {
    fn new(id: [u8; 16], location: DocLocation, len: u32, tokens: Vec<u32>) -> Self {
        Self {
            id: id,
            data: None,
            location: location,
            len: len,
            tokens: tokens,
        }
    }
}

#[pymethods]
impl Document {
    #[getter(id)]
    pub fn id(&self) -> PyResult<String> {
        Ok(Ulid::from_bytes(self.id).to_string())
    }

    #[getter(content)]
    pub fn content(&mut self) -> PyResult<String> {
        let content = match &self.data {
            Some(val) => val.clone(),
            None => {
                let DocLocation {
                    segment,
                    offset,
                    size,
                } = &self.location;

                let data = File::open(segment.join("data"))?;
                let mut buf = vec![0u8; *size];
                data.read_exact_at(&mut buf, *offset)?;
                let data = match decompress_size_prepended(&buf) {
                    Ok(data) => data,
                    Err(err) => {
                        return Err(PyValueError::new_err(format!(
                            "Failed to decompress document content: {}",
                            err
                        )));
                    }
                };
                let data = String::from_utf8(data)?;
                self.data.replace(data.clone());
                data
            }
        };

        Ok(content)
    }
}

#[derive(Decode, Encode, PartialEq, Debug, Clone)]
pub struct DocLocation {
    pub segment: PathBuf,
    pub offset: u64,
    pub size: usize,
}

#[derive(Debug, Clone)]
struct Segment {
    name: u128,
    size: u64,
    deleted: u64,
}

struct Buffer {
    segment_size: Option<u64>,
    documents: Vec<u8>,
    meta: Vec<u8>,
}

impl Buffer {
    fn new() -> Self {
        Self {
            segment_size: None,
            documents: vec![],
            meta: vec![],
        }
    }

    fn write_document(&mut self, doc: &str) -> Result<(usize, usize), DocumentBufferError> {
        // preappend document length
        self.documents.extend((doc.len() as u32).to_le_bytes());
        let offset = self.documents.len();

        self.documents
            .resize(offset + get_maximum_output_size(doc.len()), 0);
        let compressed_size = compress_into(doc.as_bytes(), &mut self.documents[offset..])?;
        self.documents.truncate(offset + compressed_size);

        // 4 bytes for extra preappended document length
        Ok((offset - 4, compressed_size + 4))
    }

    fn write_meta(&mut self, doc: &Document) -> Result<(), DocumentBufferError> {
        let config = bincode::config::standard();
        let size = {
            let mut size_writer =
                EncoderImpl::<_, Configuration>::new(SizeWriter::default(), config);
            doc.encode(&mut size_writer)?;
            size_writer.into_writer().bytes_written
        };

        self.meta.extend((size as u64).to_be_bytes());
        let offset = self.meta.len();
        self.meta.resize(offset + size, 0);

        let size = bincode::encode_into_slice(&doc, &mut self.meta[offset..], config)?;
        self.meta.truncate(offset + size);
        Ok(())
    }

    fn reset(&mut self) {
        self.documents.clear();
        self.meta.clear();
        self.segment_size.take();
    }

    fn segment_size(&mut self, segment: &PathBuf) -> Result<u64, io::Error> {
        match self.segment_size {
            Some(size) => Ok(size),
            None => {
                let file = segment.join("data");
                let data = File::options().append(true).open(&file)?;
                let size = data.metadata()?.len();
                self.segment_size.replace(size);
                Ok(size)
            }
        }
    }
}

pub struct DocumentsManager {
    pub dir: PathBuf,
    pub docs: HashMap<Ulid, Document>,
    pub deleted_docs_buffer: HashMap<Ulid, Document>,
    buffer: Buffer,
    segments: HashMap<PathBuf, Segment>,
    cur_segment: PathBuf,
    last_save: u64,
    config: Arc<Config>,
}

impl DocumentsManager {
    pub fn load(dir: PathBuf, config: Arc<Config>) -> Result<Self, DocumentsManagerError> {
        let (mut documents, mut segments_map) = (HashMap::new(), HashMap::new());

        let cur_segment = match Self::segments(&dir)? {
            Some(segments) => {
                let cur_segment = segments
                    .iter()
                    .max_by(|(_, x, _), (_, y, _)| x.name.cmp(&y.name))
                    .unwrap()
                    .0
                    .clone();

                // TODO: in future can validate segment files before loading them
                // to check if they are not malicious or corrupted
                for (path, segment, deletes) in segments {
                    let mut meta = File::open(path.join("meta"))?;
                    let meta_size = meta.metadata()?.len();

                    while meta.stream_position()? < meta_size {
                        let mut size = [0u8; 8];
                        meta.read_exact(&mut size)?;
                        let size = u64::from_be_bytes(size);
                        let mut doc = vec![0u8; size as usize];
                        meta.read_exact(&mut doc)?;
                        let (doc, _): (Document, usize) =
                            bincode::decode_from_slice(&doc, bincode::config::standard())?;

                        let ulid = Ulid::from_bytes(doc.id);
                        if deletes.contains(&ulid) {
                            continue;
                        }

                        documents.insert(Ulid::from_bytes(doc.id), doc);
                    }

                    segments_map.insert(path, segment);
                }
                cur_segment
            }
            None => {
                fs::create_dir_all(&dir)?;
                let (path, segment) = Self::create_segment(&dir)?;
                segments_map.insert(path.clone(), segment);
                path
            }
        };

        Ok(Self {
            docs: documents,
            deleted_docs_buffer: HashMap::with_capacity(100),
            dir: dir,
            buffer: Buffer::new(),
            segments: segments_map,
            cur_segment: cur_segment,
            last_save: SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)?
                .as_secs(),
            config: config,
        })
    }

    pub fn write(
        &mut self,
        id: Ulid,
        len: u32,
        tokens: Vec<u32>,
        content: &str,
    ) -> Result<(), DocumentsManagerError> {
        // write segment to buffer
        let (data_offset, size) = self.buffer.write_document(&content)?;
        let offset = self.buffer.segment_size(&self.cur_segment)? + data_offset as u64;

        let doc = Document::new(
            id.to_bytes(),
            DocLocation {
                segment: self.cur_segment.clone(),
                offset: offset,
                size: size,
            },
            len,
            tokens,
        );

        self.buffer.write_meta(&doc)?;
        self.save_buffer(offset + size as u64)?;

        self.docs.insert(id, doc);

        return Ok(());
    }

    pub fn delete(&mut self, id: Ulid) -> Result<(), io::Error> {
        let doc = match self.docs.get(&id) {
            Some(doc) => doc,
            None => return Ok(()),
        };

        let mut deletes = File::options()
            .append(true)
            .open(doc.location.segment.join("del"))?;
        deletes.write_all(&doc.id)?;
        deletes.write_all(&(doc.location.size as u64).to_be_bytes())?;
        if let Some(segment) = self.segments.get_mut(&doc.location.segment) {
            segment.deleted += doc.location.size as u64;
        }

        if let Some(doc) = self.docs.remove(&id) {
            self.deleted_docs_buffer.insert(id, doc);
        }
        Ok(())
    }

    pub fn flush(&mut self) -> Result<(), io::Error> {
        let mut data = File::options()
            .append(true)
            .open(self.cur_segment.join("data"))?;

        let mut meta = File::options()
            .append(true)
            .open(self.cur_segment.join("meta"))?;

        // flush data to disk
        data.write_all(&self.buffer.documents)?;
        meta.write_all(&self.buffer.meta)?;
        self.buffer.reset();
        Ok(())
    }

    pub fn merge(&mut self) -> Result<(), DocumentsManagerError> {
        // Merges the segments cleaning up deleted data

        let mut segments = self
            .segments
            .clone()
            .into_iter()
            .collect::<Vec<(PathBuf, Segment)>>();
        segments.sort_by(|x, y| x.1.name.cmp(&y.1.name));

        let mut merged = false;
        for (path, segment) in segments {
            merged = merged || self.merge_segment(path, segment)?;
        }

        if merged {
            self.flush()?;
        }

        Ok(())
    }

    fn merge_segment(
        &mut self,
        path: PathBuf,
        segment: Segment,
    ) -> Result<bool, DocumentsManagerError> {
        if path == self.cur_segment
            || segment.size == 0
            || (segment.deleted as f64 / segment.size as f64) < self.config.merge_deleted_ratio
        {
            return Ok(false);
        }

        let mut deletes = HashSet::new();
        let mut del = File::open(path.join("del"))?;
        let del_size = del.metadata()?.len();

        while del.stream_position()? < del_size {
            let mut ulid = [0u8; 16];
            del.read_exact(&mut ulid)?;
            del.seek_relative(8)?; // skip 'deleted size'
            deletes.insert(Ulid::from_bytes(ulid));
        }

        let data = File::open(path.join("data"))?;
        let mut meta = File::open(path.join("meta"))?;
        let meta_size = meta.metadata()?.len();

        while meta.stream_position()? < meta_size {
            let mut size_buf = [0u8; 8];
            meta.read_exact(&mut size_buf)?;
            let size = u64::from_be_bytes(size_buf);
            let mut doc_buf = vec![0u8; size as usize];
            meta.read_exact(&mut doc_buf)?;
            let (mut doc, _): (Document, usize) =
                bincode::decode_from_slice(&doc_buf, bincode::config::standard())?;

            let ulid = Ulid::from_bytes(doc.id);
            if deletes.contains(&ulid) {
                continue;
            }

            let offset = self.buffer.documents.len();
            self.buffer.documents.resize(offset + doc.location.size, 0);
            data.read_exact_at(&mut self.buffer.documents[offset..], doc.location.offset)?;

            // update in-memory document
            doc.location.segment = self.cur_segment.clone();
            doc.location.offset = self.buffer.segment_size(&self.cur_segment)? + offset as u64;

            self.buffer.write_meta(&doc)?;
            self.docs.insert(ulid, doc);

            let segment_size =
                self.buffer.segment_size(&self.cur_segment)? + self.buffer.documents.len() as u64;
            self.save_buffer(segment_size)?;
        }

        remove_dir_all(&path)?;
        self.segments.remove(&path);
        return Ok(true);
    }

    fn create_segment(dir: &PathBuf) -> Result<(PathBuf, Segment), DocumentsManagerError> {
        let ts = SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos();

        let segment = Path::new(dir).join(ts.to_string());
        fs::create_dir(&segment)?;

        for f in ["data", "meta", "del"] {
            File::create(segment.join(f))?;
        }

        Ok((
            segment,
            Segment {
                name: ts,
                size: 0,
                deleted: 0,
            },
        ))
    }

    fn segments(
        dir: &PathBuf,
    ) -> Result<Option<Vec<(PathBuf, Segment, HashSet<Ulid>)>>, io::Error> {
        match fs::exists(&dir)? {
            true => {
                let mut segments = vec![];
                for e in fs::read_dir(&dir)? {
                    let path = e?.path();
                    if !path.is_dir() || path.is_symlink() {
                        continue;
                    }

                    let name = match path
                        .file_name()
                        .unwrap_or_default()
                        .to_os_string()
                        .to_str()
                        .unwrap_or_default()
                        .parse::<u128>()
                    {
                        Ok(val) => val,
                        Err(_) => continue,
                    };

                    let data = File::open(&path.join("data"))?;
                    let mut del = File::open(path.join("del"))?;

                    let del_size = del.metadata()?.len();
                    let mut deleted_bytes = 0;
                    let mut deletes = HashSet::new();

                    while del.stream_position()? < del_size {
                        let (mut size, mut deleted) = ([0u8; 8], [0u8; 16]);
                        del.read_exact(&mut deleted)?;
                        del.read_exact(&mut size)?;

                        deletes.insert(Ulid::from_bytes(deleted));
                        deleted_bytes += u64::from_be_bytes(size);
                    }

                    segments.push((
                        path.clone(),
                        Segment {
                            name: name,
                            size: data.metadata()?.len(),
                            deleted: deleted_bytes,
                        },
                        deletes,
                    ));
                }

                if segments.len() > 0 {
                    Ok(Some(segments))
                } else {
                    Ok(None)
                }
            }
            false => Ok(None),
        }
    }

    fn save_buffer(&mut self, segment_size: u64) -> Result<(), DocumentsManagerError> {
        if let Some(segment) = self.segments.get_mut(&self.cur_segment) {
            segment.size = segment_size;
        }

        let cur_ts = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)?
            .as_secs();

        if self.buffer.documents.len() as u64 > self.config.documents_buffer_size
            || cur_ts >= self.last_save + self.config.documents_save_after_seconds
        {
            self.last_save = cur_ts;
            self.flush()?;
        }

        // check if segment size exceded threshold - 100MB
        if segment_size > self.config.segment_size {
            self.flush()?;
            let (path, segment) = Self::create_segment(&self.dir)?;
            self.segments.insert(path.clone(), segment);
            self.buffer.reset();
            self.cur_segment = path;
            self.last_save = cur_ts;
        }

        Ok(())
    }
}
