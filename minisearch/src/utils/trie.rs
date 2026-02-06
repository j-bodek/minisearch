use crate::utils::automaton::{
    LevenshteinAutomaton, LevenshteinAutomatonBuilder, LevenshteinDfaState,
};
use std::collections::HashMap;

struct Node {
    is_word: bool,
    nodes: Vec<(char, Node)>,
}

pub struct Trie {
    automaton_builders: HashMap<u8, LevenshteinAutomatonBuilder>,
    nodes: Vec<(char, Node)>,
}

impl Node {
    fn new(is_word: bool) -> Self {
        Self {
            is_word: is_word,
            nodes: Vec::new(),
        }
    }
}

impl Trie {
    pub fn new() -> Self {
        Self {
            automaton_builders: HashMap::new(),
            nodes: Vec::new(),
        }
    }

    pub fn init_automaton(&mut self, d: u8) {
        self.automaton_builders
            .insert(d, LevenshteinAutomatonBuilder::new(d));
    }

    pub fn add(&mut self, word: &str) {
        let mut nodes = &mut self.nodes;
        let len = word.chars().count();

        for (i, c) in word.chars().enumerate() {
            match nodes.binary_search_by(|t| t.0.cmp(&c)) {
                Ok(index) => {
                    if i == len - 1 {
                        nodes[index].1.is_word = true;
                    }
                    nodes = &mut nodes[index].1.nodes;
                }
                Err(index) => {
                    let node = Node::new(i == len - 1);
                    nodes.insert(index, (c, node));
                    nodes = &mut nodes[index].1.nodes;
                }
            }
        }
    }

    pub fn delete(&mut self, word: String) {
        let mut chars: Vec<char> = word.chars().rev().collect();
        Self::_delete(&mut chars, &mut self.nodes);
    }

    pub fn search(&self, d: u8, query: &str) -> Vec<(u16, String)> {
        match self.automaton_builders.get(&d) {
            Some(builder) => {
                let mut automaton = builder.get(query);
                let state = automaton.initial_state();
                let mut prefix = String::new();
                let mut matches = Vec::new();
                self._search(
                    &mut prefix,
                    &mut matches,
                    &self.nodes,
                    &state,
                    &mut automaton,
                );
                matches
            }
            None => vec![],
        }
    }
}

impl Trie {
    fn _delete(chars: &mut Vec<char>, nodes: &mut Vec<(char, Node)>) -> (usize, bool, bool) {
        if chars.len() == 0 {
            return (0, false, true);
        }

        if let Ok(index) = nodes.binary_search_by(|t| t.0.cmp(&chars[chars.len() - 1])) {
            chars.pop();
            let node = &mut nodes.get_mut(index).unwrap().1;

            if chars.len() == 0 {
                node.is_word = false;
                return (index, node.nodes.len() == 0, true);
            } else {
                let (idx, can_remove, deleted) = Self::_delete(chars, &mut node.nodes);
                if can_remove && deleted {
                    node.nodes.remove(idx);
                    return (
                        index,
                        node.nodes.len() == 0 && node.is_word == false,
                        deleted,
                    );
                }
                return (index, false, deleted);
            }
        }

        return (0, false, false);
    }

    fn _search(
        &self,
        prefix: &mut String,
        matches: &mut Vec<(u16, String)>,
        nodes: &Vec<(char, Node)>,
        state: &LevenshteinDfaState,
        automaton: &mut LevenshteinAutomaton,
    ) {
        for (c, node) in nodes.iter() {
            let new_state = automaton.step(*c, &state);
            if !automaton.can_match(&new_state) {
                continue;
            }

            prefix.push(*c);
            if node.is_word && automaton.is_match(&new_state) {
                matches.push((automaton.distance(&new_state), prefix.clone()));
            }

            self._search(prefix, matches, &node.nodes, &new_state, automaton);
            prefix.pop();
        }
    }
}
