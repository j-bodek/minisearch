use pyo3::prelude::*;
use regex::Regex;
use std::collections::HashMap;

static VOWELS: [&str; 6] = ["a", "e", "i", "o", "u", "y"];
static DOUBLES: [&str; 9] = ["bb", "dd", "ff", "gg", "mm", "nn", "pp", "rr", "tt"];
static LI_ENDINGS: [&str; 10] = ["c", "d", "e", "g", "h", "k", "m", "n", "r", "t"];
static EXCEPTION_WORDS: [&str; 7] = ["sky", "news", "howe", "atlas", "cosmos", "bias", "andes"];

#[pyclass(name = "SnowballStemmer")]
pub struct SnowballStemmer {
    r1: usize,
    r2: usize,
    pre_stem_exceptions: HashMap<String, String>,
    step_2_suffix_map: HashMap<String, String>,
    step_3_suffix_map: HashMap<String, String>,
}

#[pymethods]
impl SnowballStemmer {
    #[new]
    fn new() -> Self {
        let pre_stem_exceptions: HashMap<String, String> = HashMap::from([
            (String::from("skis"), String::from("ski")),
            (String::from("skies"), String::from("sky")),
            (String::from("idly"), String::from("idl")),
            (String::from("gently"), String::from("gentl")),
            (String::from("ugly"), String::from("ugli")),
            (String::from("early"), String::from("earli")),
            (String::from("only"), String::from("onli")),
            (String::from("singly"), String::from("singl")),
            (String::from("sky"), String::from("sky")),
            (String::from("news"), String::from("news")),
            (String::from("howe"), String::from("howe")),
        ]);

        let step_2_suffix_map: HashMap<String, String> = HashMap::from([
            (String::from("ization"), String::from("ize")),
            (String::from("ational"), String::from("ate")),
            (String::from("fulness"), String::from("ful")),
            (String::from("ousness"), String::from("ous")),
            (String::from("iveness"), String::from("ive")),
            (String::from("tional"), String::from("tion")),
            (String::from("biliti"), String::from("ble")),
            (String::from("lessli"), String::from("less")),
            (String::from("entli"), String::from("ent")),
            (String::from("ation"), String::from("ate")),
            (String::from("alism"), String::from("al")),
            (String::from("aliti"), String::from("al")),
            (String::from("ousli"), String::from("ous")),
            (String::from("iviti"), String::from("ive")),
            (String::from("ogist"), String::from("og")),
            (String::from("fulli"), String::from("ful")),
            (String::from("enci"), String::from("ence")),
            (String::from("anci"), String::from("ance")),
            (String::from("abli"), String::from("able")),
            (String::from("izer"), String::from("ize")),
            (String::from("ator"), String::from("ate")),
            (String::from("alli"), String::from("al")),
            (String::from("bli"), String::from("ble")),
            (String::from("ogi"), String::from("og")),
            (String::from("li"), String::from("")),
        ]);

        let step_3_suffix_map: HashMap<String, String> = HashMap::from([
            (String::from("ational"), String::from("ate")),
            (String::from("tional"), String::from("tion")),
            (String::from("alize"), String::from("al")),
            (String::from("icate"), String::from("ic")),
            (String::from("iciti"), String::from("ic")),
            (String::from("ative"), String::from("")),
            (String::from("ical"), String::from("ic")),
            (String::from("ness"), String::from("")),
            (String::from("ful"), String::from("")),
        ]);

        SnowballStemmer {
            r1: 0,
            r2: 0,
            pre_stem_exceptions: pre_stem_exceptions,
            step_2_suffix_map: step_2_suffix_map,
            step_3_suffix_map: step_3_suffix_map,
        }
    }

    fn stem(&mut self, mut word: String) -> String {
        if word.len() <= 2 || EXCEPTION_WORDS.contains(&word.as_str()) {
            return word;
        }

        self.remove_initial_apostrophe(&mut word);
        match self.pre_stem_exceptions.get(&word) {
            Some(word) => return word.to_string(),
            None => (),
        }

        self.set_ys(&mut word);
        self.find_r1r2(&mut word);

        return word;
    }
}

// impl not exposed to python
impl SnowballStemmer {
    fn remove_initial_apostrophe(&self, word: &mut String) {
        if word.starts_with("'") {
            let _ = &word.remove(0);
        }
    }

    fn set_ys(&self, word: &mut String) {
        if word.starts_with("y") {
            let _ = word.replacen("y", "Y", 1);
        }

        let matches: Vec<usize> = Regex::new(r"[aeiou]y")
            .unwrap()
            .find_iter(word)
            .map(|m| m.end())
            .collect();

        for m in matches {
            word.replace_range(m - 1..m, "Y");
        }
    }

    fn find_r1r2(&mut self, word: &mut String) {
        self.r1 = word.len();
        self.r2 = word.len();

        let prefix = Regex::new(r"^(gener|commun|arsen|past|univers|later|emerg|organ)")
            .unwrap()
            .find(word);

        match prefix {
            Some(prefix) => {
                let prefix = prefix.as_str();
                self.r1 = prefix.len();

                let matches: Vec<usize> = Regex::new(r"[aeiouy][^aeiouy]")
                    .unwrap()
                    .find_iter(&word[self.r1..])
                    .map(|m| m.end())
                    .collect();

                for m in matches {
                    self.r2 = self.r1 + m - 1;
                    break;
                }
            }
            None => {
                let matches: Vec<usize> = Regex::new(r"[aeiouy][^aeiouy]")
                    .unwrap()
                    .find_iter(word)
                    .map(|m| m.end())
                    .collect();

                for (index, &m) in matches.iter().enumerate() {
                    if index == 0 {
                        self.r1 = m - 1;
                    } else if index == 1 {
                        self.r2 = m - 1;
                        break;
                    }
                }
            }
        }
    }
}
