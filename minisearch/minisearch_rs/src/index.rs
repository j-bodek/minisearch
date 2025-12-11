use std::{io, path::PathBuf};

use hashbrown::HashMap;

use ulid::Ulid;

pub struct Posting {
    pub doc_id: Ulid,
    pub positions: Vec<u32>,
    pub score: f64,
}

pub struct IndexManager {
    pub index: HashMap<u32, Vec<Posting>>,
}

impl IndexManager {
    pub fn load(dir: &PathBuf) -> Result<Self, io::Error> {
        // TODO: implement load functionality
        Ok(Self {
            index: HashMap::new(),
        })
    }
}
