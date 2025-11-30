use hashbrown::HashMap;

pub struct TokenHasher {
    map: HashMap<String, u32>,
    tokens: Vec<Option<String>>,
    deleted: Vec<u32>,
}

impl TokenHasher {
    pub fn new() -> Self {
        Self {
            map: HashMap::new(),
            tokens: Vec::new(),
            deleted: Vec::new(),
        }
    }

    pub fn add(&mut self, token: String) -> u32 {
        if let Some(idx) = self.map.get(&token) {
            return *idx;
        }

        let idx = if !self.deleted.is_empty() {
            let idx = self.deleted.pop().unwrap();
            self.tokens[idx as usize] = Some(token.clone());
            idx
        } else {
            self.tokens.push(Some(token.clone()));
            self.tokens.len() as u32
        };

        self.map.insert(token, idx);
        return idx;
    }

    pub fn hash(&self, token: &String) -> Option<u32> {
        match self.map.get(token) {
            Some(idx) => Some(*idx),
            None => None,
        }
    }

    pub fn unhash(&self, token: u32) -> Option<&String> {
        match self.tokens.get(token as usize) {
            Some(val) => val.as_ref(),
            None => None,
        }
    }

    pub fn delete(&mut self, token: u32) -> Option<String> {
        if token as usize >= self.tokens.len() || self.tokens[token as usize].is_none() {
            return None;
        }

        let token_str = self.tokens[token as usize].take().unwrap();
        self.deleted.push(token);
        self.map.remove(&token_str);
        Some(token_str)
    }
}
