use crate::index::Document;
use bincode::{Decode, Encode};
use hashbrown::{HashMap, HashSet};
use lz4_flex::block::{compress_prepend_size, decompress_size_prepended};
use std::error::Error;
use std::io::{self, prelude::*};
use std::os::unix::prelude::FileExt;
use std::{
    fs::{self, File},
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};
use ulid::Ulid;

static SEGMENT_THRESHOLD: u64 = 100 * 1024 * 1024;

#[derive(Decode, Encode, PartialEq, Debug)]
pub struct DocLocation {
    pub segment: PathBuf,
    pub offset: u64,
    pub size: usize,
}

pub struct DocumentsWriter {
    pub dir: PathBuf,
    cur_segment: PathBuf,
}

impl DocumentsWriter {
    pub fn new(dir: PathBuf) -> Result<Self, io::Error> {
        let cur_segment = match fs::exists(&dir)? {
            true => {
                let mut segment: u64 = 0;
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
                        .parse::<u64>()
                    {
                        Ok(val) => val,
                        Err(_) => 0,
                    };

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
            cur_segment: cur_segment,
        })
    }

    fn create_segment(dir: &PathBuf) -> Result<PathBuf, io::Error> {
        let ts = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs()
            .to_string();

        let segment = Path::new(dir).join(ts);
        fs::create_dir(&segment)?;

        for f in ["segment", "meta", "del"] {
            File::create(segment.join(f))?;
        }

        Ok(segment)
    }

    pub fn load(dir: &PathBuf) -> Result<(HashMap<Ulid, Document>, HashSet<Ulid>), io::Error> {
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

                    documents.insert(Ulid::from_bytes(doc.id), doc);
                }

                let del = path.join("del");
                let mut del = File::open(del)?;
                let del_size = del.metadata()?.len();

                while del.stream_position().unwrap() < del_size {
                    let mut ulid = [0u8; 16];
                    del.read_exact(&mut ulid)?;
                    deletes.insert(Ulid::from_bytes(ulid));
                }
            }
        };

        return Ok((documents, deletes));
    }

    pub fn write(
        &mut self,
        id: Ulid,
        tokens: Vec<u32>,
        content: &str,
    ) -> Result<Document, io::Error> {
        // update segment and meta file - use LZ4 compression
        let file = self.cur_segment.join("segment");
        let mut segment = File::options().append(true).open(&file)?;
        let content = compress_prepend_size(content.as_bytes());
        let (offset, size) = (segment.metadata()?.len(), content.len());
        segment.write_all(&content)?;

        let segment = self.cur_segment.clone();
        let doc = Document {
            id: id.to_bytes(),
            location: DocLocation {
                segment: segment,
                offset: offset,
                size: size,
            },
            tokens,
        };

        let file = self.cur_segment.join("meta");
        let mut meta = File::options().append(true).open(&file)?;
        let data = bincode::encode_to_vec(&doc, bincode::config::standard()).unwrap();
        let data_size = (data.len() as u64).to_be_bytes();
        meta.write_all(&data_size)?;
        meta.write_all(&data)?;

        // check if segment size exceded threshold - 100MB
        if offset + size as u64 > SEGMENT_THRESHOLD {
            self.cur_segment = Self::create_segment(&self.dir)?;
        }

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

    pub fn delete(&self, id: Ulid) -> Result<(), io::Error> {
        let file = self.cur_segment.join("del");
        let mut deletes = File::options().append(true).open(&file)?;
        deletes.write_all(&id.to_bytes())?;
        Ok(())
    }
}
