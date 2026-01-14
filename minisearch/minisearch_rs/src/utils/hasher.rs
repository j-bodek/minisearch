use std::{
    collections::hash_map::Keys,
    fs::{self, File},
    io,
    path::PathBuf,
    time::SystemTime,
};

use bincode::{Decode, Encode};
use std::collections::HashMap;

use crate::errors::BincodePersistenceError;

static OPERATIONS_THRESHOLD: u32 = 100_000;
static SAVE_SECS_THRESHOLD: u64 = 5;

#[derive(Decode, Encode, PartialEq, Debug, Clone)]
struct TokensStore {
    map: HashMap<String, u32>,
    tokens: Vec<Option<String>>,
    deleted: Vec<u32>,
}

impl TokensStore {
    fn new(map: HashMap<String, u32>, tokens: Vec<Option<String>>, deleted: Vec<u32>) -> Self {
        Self {
            map: map,
            tokens: tokens,
            deleted: deleted,
        }
    }

    fn load(path: &PathBuf) -> Result<Self, io::Error> {
        if !fs::exists(path)? {
            File::create(path)?;
            return Ok(Self::new(HashMap::new(), Vec::new(), Vec::new()));
        }

        let mut file = File::open(path)?;
        // if file is empty don't try to decode tokens
        if file.metadata()?.len() == 0 {
            return Ok(Self::new(HashMap::new(), Vec::new(), Vec::new()));
        }

        match bincode::decode_from_std_read(&mut file, bincode::config::standard()) {
            Ok(store) => Ok(store),
            Err(e) => {
                println!("Warning tokens decode error: {e}");
                Ok(Self::new(HashMap::new(), Vec::new(), Vec::new()))
            }
        }
    }
}

pub struct TokenHasher {
    path: PathBuf,
    operations: u32,
    last_save: u64,
    tokens_store: TokensStore,
}

impl TokenHasher {
    pub fn load(dir: &PathBuf) -> Result<Self, BincodePersistenceError> {
        let index_dir = dir.join("index");
        let tokens = index_dir.join("tokens");
        if !fs::exists(&index_dir)? || !fs::exists(&tokens)? {
            fs::create_dir_all(&index_dir)?;
            File::create(&tokens)?;
        }

        Ok(Self {
            tokens_store: TokensStore::load(&tokens)?,
            path: tokens,
            operations: 0,
            last_save: SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)?
                .as_secs(),
        })
    }

    pub fn tokens(&self) -> Keys<'_, String, u32> {
        self.tokens_store.map.keys()
    }

    pub fn add(&mut self, token: String) -> Result<u32, BincodePersistenceError> {
        if let Some(idx) = self.tokens_store.map.get(&token) {
            return Ok(*idx);
        }

        let idx = if let Some(idx) = self.tokens_store.deleted.pop() {
            self.tokens_store.tokens[idx as usize] = Some(token.clone());
            idx
        } else {
            self.tokens_store.tokens.push(Some(token.clone()));
            (self.tokens_store.tokens.len() - 1) as u32
        };

        self.tokens_store.map.insert(token, idx);
        self.operations += 1;
        self.save()?;
        return Ok(idx);
    }

    pub fn delete(&mut self, token: u32) -> Result<Option<String>, BincodePersistenceError> {
        if let Some(token_str) = self.tokens_store.tokens.get_mut(token as usize)
            && let Some(token_str) = token_str.take()
        {
            self.tokens_store.deleted.push(token);
            self.tokens_store.map.remove(&token_str);
            self.operations += 1;
            self.save()?;
            return Ok(Some(token_str));
        }

        Ok(None)
    }

    pub fn hash(&self, token: &String) -> Option<u32> {
        match self.tokens_store.map.get(token) {
            Some(idx) => Some(*idx),
            None => None,
        }
    }

    pub fn unhash(&self, token: u32) -> Option<&String> {
        match self.tokens_store.tokens.get(token as usize) {
            Some(val) => val.as_ref(),
            None => None,
        }
    }

    fn save(&mut self) -> Result<(), BincodePersistenceError> {
        let cur_ts = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)?
            .as_secs();

        if self.operations >= OPERATIONS_THRESHOLD || cur_ts >= self.last_save + SAVE_SECS_THRESHOLD
        {
            self.operations = 0;
            self.last_save = cur_ts;
            self.flush()?;
        }

        Ok(())
    }

    pub fn flush(&self) -> Result<(), BincodePersistenceError> {
        let mut file = File::create(&self.path)?;
        bincode::encode_into_std_write(&self.tokens_store, &mut file, bincode::config::standard())?;
        Ok(())
    }
}
