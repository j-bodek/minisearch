use crate::index::Document;
use bincode::config::Configuration;
use bincode::enc::write::SizeWriter;
use bincode::enc::EncoderImpl;
use bincode::{Decode, Encode};
use hashbrown::{HashMap, HashSet};
use lz4_flex::block::{compress_into, decompress_size_prepended, get_maximum_output_size};
use std::error::Error;
use std::fs::remove_dir_all;
use std::hash::Hash;
use std::io::{self, prelude::*};
use std::os::unix::prelude::FileExt;
use std::{
    fs::{self, File},
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};
use ulid::Ulid;

static SEGMENT_THRESHOLD: u64 = 1 * 1024 * 1024;
static DOCUMENTS_BUFFER_THRESHOLD: u64 = 1024 * 1024;
static MERGE_THRESHOLD: f64 = 0.3;

#[derive(Decode, Encode, PartialEq, Debug)]
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
        let offset = self.documents.len();
        self.documents
            .resize(offset + get_maximum_output_size(doc.len()), 0);
        let size = compress_into(doc.as_bytes(), &mut self.documents[offset..]).unwrap();
        self.documents.truncate(offset + size);

        (offset, size)
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
                let file = segment.join("segment");
                let segment = File::options().append(true).open(&file)?;
                let size = segment.metadata()?.len();
                self.segment_size.replace(size);
                Ok(size)
            }
        }
    }
}

pub struct DocumentsWriter {
    pub dir: PathBuf,
    buffer: Buffer,
    segments: HashMap<PathBuf, Segment>,
    cur_segment: PathBuf,
}

impl DocumentsWriter {
    pub fn new(dir: PathBuf) -> Result<Self, io::Error> {
        let mut segments = HashMap::new();
        let cur_segment = match fs::exists(&dir)? {
            true => {
                let mut segment = 0;
                let mut segment_path = None;
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

                    let segment_file = File::open(&path.join("segment"))?;

                    let mut del = File::open(path.join("del"))?;
                    let del_size = del.metadata()?.len();
                    let mut deleted = 0;

                    while del.stream_position().unwrap() < del_size {
                        let mut size = [0u8; 8];
                        del.seek_relative(16)?; // skip 'ulid'
                        del.read_exact(&mut size)?;
                        deleted += u64::from_be_bytes(size);
                    }

                    segments.insert(
                        path.clone(),
                        Segment {
                            name: name,
                            size: segment_file.metadata()?.len(),
                            deleted: deleted,
                        },
                    );

                    if name > segment {
                        segment = name;
                        segment_path = Some(path);
                    }
                }

                match segment_path {
                    Some(path) => path,
                    None => Self::create_segment(&dir)?,
                }
            }
            false => {
                fs::create_dir_all(&dir)?;
                Self::create_segment(&dir)?
            }
        };

        Ok(Self {
            dir: dir,
            buffer: Buffer::new(),
            segments: segments,
            cur_segment: cur_segment,
        })
    }

    fn create_segment(dir: &PathBuf) -> Result<PathBuf, io::Error> {
        // TODO: append new segment to segments
        let ts = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
            .to_string();

        let segment = Path::new(dir).join(ts);
        fs::create_dir(&segment)?;

        for f in ["segment", "meta", "del"] {
            File::create(segment.join(f))?;
        }

        Ok(segment)
    }

    pub fn load(dir: &PathBuf) -> Result<HashMap<Ulid, Document>, io::Error> {
        let (mut documents, mut deletes) = (HashMap::new(), HashSet::new());
        if fs::exists(&dir)? {
            for e in fs::read_dir(&dir)? {
                let path = e?.path();
                if !path.is_dir() {
                    continue;
                }

                // TODO: properly validate if dir is timestamp
                if let Err(_) = path
                    .file_name()
                    .unwrap()
                    .to_os_string()
                    .to_str()
                    .unwrap()
                    .parse::<u64>()
                {
                    continue;
                };

                let del = path.join("del");
                let mut del = File::open(del)?;
                let del_size = del.metadata()?.len();

                while del.stream_position().unwrap() < del_size {
                    let mut ulid = [0u8; 16];
                    del.read_exact(&mut ulid)?;
                    del.seek_relative(8)?; // skip 'deleted size'
                    deletes.insert(Ulid::from_bytes(ulid));
                }

                let meta = path.join("meta");
                let mut meta = File::open(meta)?;
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
            }
        };

        return Ok(documents);
    }

    pub fn write(
        &mut self,
        id: Ulid,
        tokens: Vec<u32>,
        content: &str,
    ) -> Result<Document, Box<dyn Error>> {
        // write segment to buffer
        let (data_offset, size) = self.buffer.write_document(&content);
        let offset = self.buffer.segment_size(&self.cur_segment)? + data_offset as u64;

        let doc = Document {
            id: id.to_bytes(),
            location: DocLocation {
                segment: self.cur_segment.clone(),
                offset: offset,
                size: size,
            },
            tokens,
        };
        self.buffer.write_meta(&doc)?;
        self.save_buffer(offset + size as u64)?;

        return Ok(doc);
    }

    pub fn read(&self, doc: &Document) -> Result<String, Box<dyn Error>> {
        let DocLocation {
            segment,
            offset,
            size,
        } = &doc.location;

        let file = segment.join("segment");
        let segment = File::open(&file)?;
        let mut buf = vec![0u8; *size];
        segment.read_at(&mut buf, *offset)?;
        let data = decompress_size_prepended(&buf)?;

        Ok(String::from_utf8(data)?)
    }

    pub fn delete(&mut self, doc: &Document) -> Result<(), io::Error> {
        let file = doc.location.segment.join("del");
        let mut deletes = File::options().append(true).open(&file)?;
        deletes.write_all(&doc.id)?;
        deletes.write_all(&(doc.location.size as u64).to_be_bytes())?;
        if let Some(segment) = self.segments.get_mut(&doc.location.segment) {
            segment.deleted += doc.location.size as u64;
        }
        Ok(())
    }

    pub fn flush(&mut self) -> Result<(), io::Error> {
        let file = self.cur_segment.join("segment");
        let mut segment = File::options().append(true).open(&file)?;

        let file = self.cur_segment.join("meta");
        let mut meta = File::options().append(true).open(&file)?;

        // flush data to disk
        segment.write_all(&self.buffer.documents)?;
        meta.write_all(&self.buffer.meta)?;
        self.buffer.reset();
        Ok(())
    }

    pub fn clean(&mut self) -> Result<(), io::Error> {
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
            let del = path.join("del");
            let mut del = File::open(del)?;
            let del_size = del.metadata()?.len();

            while del.stream_position().unwrap() < del_size {
                let mut ulid = [0u8; 16];
                del.read_exact(&mut ulid)?;
                del.seek_relative(8)?; // skip 'deleted size'
                deletes.insert(Ulid::from_bytes(ulid));
            }

            let file = path.join("segment");
            let segment = File::open(&file)?;

            let meta = path.join("meta");
            let mut meta = File::open(meta)?;
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
                segment.read_at(&mut self.buffer.documents[offset..], doc.location.offset)?;

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

    fn save_buffer(&mut self, segment_size: u64) -> Result<(), io::Error> {
        if self.buffer.documents.len() as u64 > DOCUMENTS_BUFFER_THRESHOLD {
            self.flush()?;
        }

        // check if segment size exceded threshold - 100MB
        if segment_size > SEGMENT_THRESHOLD {
            self.flush()?;
            self.cur_segment = Self::create_segment(&self.dir)?;
        }

        Ok(())
    }
}
