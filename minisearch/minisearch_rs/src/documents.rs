use bincode::config::Configuration;
use bincode::enc::write::SizeWriter;
use bincode::enc::EncoderImpl;
use bincode::{Decode, Encode};
use hashbrown::{HashMap, HashSet};
use lz4_flex::block::{compress_into, decompress_size_prepended, get_maximum_output_size};
use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;
use std::error::Error;
use std::fs::remove_dir_all;
use std::io::{self, prelude::*};
use std::os::unix::prelude::FileExt;
use std::{
    fs::{self, File},
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};
use ulid::Ulid;

static SEGMENT_THRESHOLD: u64 = 50 * 1024 * 1024;
static DOCUMENTS_BUFFER_THRESHOLD: u64 = 1024 * 1024;
static MERGE_THRESHOLD: f64 = 0.3;

#[pyclass(name = "Document")]
#[derive(Decode, Encode, PartialEq, Debug, Clone)]
pub struct Document {
    pub id: [u8; 16], // binary representation of ULID
    data: Option<String>,
    pub location: DocLocation,
    pub tokens: Vec<u32>,
}

impl Document {
    fn new(id: [u8; 16], location: DocLocation, tokens: Vec<u32>) -> Self {
        Self {
            id: id,
            data: None,
            location: location,
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
                data.read_at(&mut buf, *offset)?;
                let data = match decompress_size_prepended(&buf) {
                    Ok(data) => data,
                    Err(err) => {
                        return Err(PyValueError::new_err(format!(
                            "Failed to decompress document content: {}",
                            err
                        )))
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

    fn write_document(&mut self, doc: &str) -> (usize, usize) {
        // preappend document length
        self.documents.extend((doc.len() as u32).to_le_bytes());
        let offset = self.documents.len();

        self.documents
            .resize(offset + get_maximum_output_size(doc.len()), 0);
        let compressed_size = compress_into(doc.as_bytes(), &mut self.documents[offset..]).unwrap();
        self.documents.truncate(offset + compressed_size);

        // 4 bytes for extra preappended document length
        (offset - 4, compressed_size + 4)
    }

    fn write_meta(&mut self, doc: &Document) -> Result<(), Box<dyn Error>> {
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

        let size = bincode::encode_into_slice(&doc, &mut self.meta[offset..], config).unwrap();
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
    buffer: Buffer,
    segments: HashMap<PathBuf, Segment>,
    cur_segment: PathBuf,
}

impl DocumentsManager {
    pub fn load(dir: PathBuf) -> Result<Self, io::Error> {
        let (mut documents, mut segments_map) = (HashMap::new(), HashMap::new());

        let cur_segment = match Self::segments(&dir)? {
            Some(segments) => {
                let mut deletes = HashSet::new();
                let cur_segment = segments
                    .iter()
                    .max_by(|(_, x), (_, y)| x.name.cmp(&y.name))
                    .unwrap()
                    .0
                    .clone();

                for (path, segment) in segments {
                    let mut del = File::open(path.join("del"))?;
                    let del_size = del.metadata()?.len();

                    while del.stream_position().unwrap() < del_size {
                        let mut ulid = [0u8; 16];
                        del.read_exact(&mut ulid)?;
                        del.seek_relative(8)?; // skip 'deleted size'
                        deletes.insert(Ulid::from_bytes(ulid));
                    }

                    let mut meta = File::open(path.join("meta"))?;
                    let meta_size = meta.metadata()?.len();

                    while meta.stream_position().unwrap() < meta_size {
                        let mut size = [0u8; 8];
                        meta.read_exact(&mut size)?;
                        let size = u64::from_be_bytes(size);
                        let mut doc = vec![0u8; size as usize];
                        meta.read_exact(&mut doc)?;
                        let (doc, _): (Document, usize) =
                            bincode::decode_from_slice(&doc, bincode::config::standard()).unwrap();

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
            dir: dir,
            buffer: Buffer::new(),
            segments: segments_map,
            cur_segment: cur_segment,
        })
    }

    pub fn len(&self) -> usize {
        return self.docs.len();
    }

    pub fn get(&self, id: &Ulid) -> Option<&Document> {
        self.docs.get(id)
    }

    pub fn insert(&mut self, id: Ulid, doc: Document) -> Option<Document> {
        self.docs.insert(id, doc)
    }

    pub fn remove(&mut self, id: &Ulid) -> Option<Document> {
        self.docs.remove(id)
    }

    pub fn write(
        &mut self,
        id: Ulid,
        tokens: Vec<u32>,
        content: &str,
    ) -> Result<(), Box<dyn Error>> {
        // write segment to buffer
        let (data_offset, size) = self.buffer.write_document(&content);
        let offset = self.buffer.segment_size(&self.cur_segment)? + data_offset as u64;

        let doc = Document::new(
            id.to_bytes(),
            DocLocation {
                segment: self.cur_segment.clone(),
                offset: offset,
                size: size,
            },
            tokens,
        );

        self.buffer.write_meta(&doc)?;
        self.save_buffer(offset + size as u64)?;

        self.docs.insert(id, doc);

        return Ok(());
    }

    pub fn delete(&mut self, id: &Ulid) -> Result<(), io::Error> {
        // TODO: buffer deletes, move deleted_documents to documents manager
        let doc = match self.docs.get(id) {
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

    pub fn merge(&mut self) -> Result<(), io::Error> {
        // Merges the segments cleaning up deleted data

        let mut segments = self
            .segments
            .clone()
            .into_iter()
            .collect::<Vec<(PathBuf, Segment)>>();
        segments.sort_by(|x, y| x.1.name.cmp(&y.1.name));

        for (path, segment) in segments {
            if (segment.deleted as f64 / segment.size as f64) < MERGE_THRESHOLD {
                continue;
            }

            let mut deletes = HashSet::new();
            let mut del = File::open(path.join("del"))?;
            let del_size = del.metadata()?.len();

            while del.stream_position().unwrap() < del_size {
                let mut ulid = [0u8; 16];
                del.read_exact(&mut ulid)?;
                del.seek_relative(8)?; // skip 'deleted size'
                deletes.insert(Ulid::from_bytes(ulid));
            }

            let data = File::open(path.join("data"))?;
            let mut meta = File::open(path.join("meta"))?;
            let meta_size = meta.metadata()?.len();

            while meta.stream_position().unwrap() < meta_size {
                let mut size_buf = [0u8; 8];
                meta.read_exact(&mut size_buf)?;
                let size = u64::from_be_bytes(size_buf);
                let mut doc_buf = vec![0u8; size as usize];
                meta.read_exact(&mut doc_buf)?;
                let (doc, _): (Document, usize) =
                    bincode::decode_from_slice(&doc_buf, bincode::config::standard()).unwrap();

                let ulid = Ulid::from_bytes(doc.id);
                if deletes.contains(&ulid) {
                    continue;
                }

                let offset = self.buffer.documents.len();
                self.buffer.documents.resize(offset + doc.location.size, 0);
                data.read_at(&mut self.buffer.documents[offset..], doc.location.offset)?;

                self.buffer.meta.extend(size_buf);
                self.buffer.meta.extend(doc_buf);

                let segment_size = self.buffer.segment_size(&self.cur_segment)?
                    + self.buffer.documents.len() as u64;
                self.save_buffer(segment_size)?;
            }

            remove_dir_all(&path)?;
            self.segments.remove(&path);
        }
        self.flush()?;

        Ok(())
    }

    fn create_segment(dir: &PathBuf) -> Result<(PathBuf, Segment), io::Error> {
        let ts = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();

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

    fn segments(dir: &PathBuf) -> Result<Option<Vec<(PathBuf, Segment)>>, io::Error> {
        match fs::exists(&dir)? {
            true => {
                let mut segments = vec![];
                for e in fs::read_dir(&dir)? {
                    let path = e?.path();
                    if !path.is_dir() {
                        continue;
                    }

                    let name = match path
                        .file_name()
                        .unwrap()
                        .to_os_string()
                        .to_str()
                        .unwrap()
                        .parse::<u128>()
                    {
                        Ok(val) => val,
                        Err(_) => continue,
                    };

                    let data = File::open(&path.join("data"))?;
                    let mut del = File::open(path.join("del"))?;

                    let del_size = del.metadata()?.len();
                    let mut deleted = 0;

                    while del.stream_position().unwrap() < del_size {
                        let mut size = [0u8; 8];
                        del.seek_relative(16)?; // skip 'ulid'
                        del.read_exact(&mut size)?;
                        deleted += u64::from_be_bytes(size);
                    }

                    segments.push((
                        path.clone(),
                        Segment {
                            name: name,
                            size: data.metadata()?.len(),
                            deleted: deleted,
                        },
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

    fn save_buffer(&mut self, segment_size: u64) -> Result<(), io::Error> {
        if let Some(segment) = self.segments.get_mut(&self.cur_segment) {
            segment.size = segment_size;
        }

        if self.buffer.documents.len() as u64 > DOCUMENTS_BUFFER_THRESHOLD {
            self.flush()?;
        }

        // check if segment size exceded threshold - 100MB
        if segment_size > SEGMENT_THRESHOLD {
            self.flush()?;
            let (path, segment) = Self::create_segment(&self.dir)?;
            self.segments.insert(path.clone(), segment);
            self.cur_segment = path;
        }

        Ok(())
    }
}
