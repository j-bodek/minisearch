use std::collections::HashMap;

static VOWELS: [char; 6] = ['a', 'e', 'i', 'o', 'u', 'y'];
static DOUBLES: [&str; 9] = ["bb", "dd", "ff", "gg", "mm", "nn", "pp", "rr", "tt"];
static LI_ENDINGS: [char; 10] = ['c', 'd', 'e', 'g', 'h', 'k', 'm', 'n', 'r', 't'];
static EXCEPTION_WORDS: [&str; 7] = ["sky", "news", "howe", "atlas", "cosmos", "bias", "andes"];
static R1_BEGININGS: [&str; 8] = [
    "gener", "commun", "arsen", "past", "univers", "later", "emerg", "organ",
];

static STEP_1A_SUFFIXES: [&str; 6] = ["sses", "ied", "ies", "us", "ss", "s"];
static STEP_1B_SUFFIXES_1: [&str; 2] = ["eedly", "eed"];
static STEP_1B_SUFFIXES_2: [&str; 4] = ["ingly", "edly", "ing", "ed"];
static STEP_2_SUFFIXES: [&str; 25] = [
    "ization", "ational", "fulness", "ousness", "iveness", "tional", "biliti", "lessli", "entli",
    "ation", "alism", "aliti", "ousli", "iviti", "ogist", "fulli", "enci", "anci", "abli", "izer",
    "ator", "alli", "bli", "ogi", "li",
];

static STEP_3_SUFFIXES: [&str; 9] = [
    "ational", "tional", "alize", "icate", "iciti", "ative", "ical", "ness", "ful",
];
static STEP_4_SUFFIXES: [&str; 18] = [
    "ement", "ance", "ence", "able", "ible", "ment", "ant", "ent", "ism", "ate", "iti", "ous",
    "ive", "ize", "ion", "al", "er", "ic",
];

pub struct SnowballStemmer {
    r1: usize,
    r2: usize,
    pre_stem_exceptions: HashMap<String, String>,
    step_2_suffix_map: HashMap<String, String>,
    step_3_suffix_map: HashMap<String, String>,
}

impl SnowballStemmer {
    pub fn new() -> Self {
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

    pub fn stem(&mut self, mut word: String) -> String {
        if word.len() <= 2 || EXCEPTION_WORDS.contains(&word.as_str()) || !word.is_ascii() {
            return word;
        }

        self.remove_initial_apostrophe(&mut word);
        match self.pre_stem_exceptions.get(&word) {
            Some(word) => return word.to_string(),
            None => (),
        }

        self.set_ys(&mut word);
        self.find_r1r2(&mut word);

        self.step_0(&mut word);
        self.step_1a(&mut word);
        self.step_1b(&mut word);
        self.step_1c(&mut word);
        self.step_2(&mut word);
        self.step_3(&mut word);
        self.step_4(&mut word);
        self.step_5(&mut word);
        word.replace("Y", "y")
    }
}

// impl not exposed to python
impl SnowballStemmer {
    fn ends_with_short_syllabe(&self, word: &str) -> bool {
        if word == "past" {
            return true;
        } else if word.len() > 2
            && !VOWELS.contains(&word.chars().nth(word.len() - 3).unwrap())
            && VOWELS.contains(&word.chars().nth(word.len() - 2).unwrap())
            && !['a', 'e', 'i', 'o', 'u', 'w', 'x', 'Y']
                .contains(&word.chars().nth(word.len() - 1).unwrap())
        {
            return true;
        } else if word.len() == 2
            && VOWELS.contains(&word.chars().nth(0).unwrap())
            && !VOWELS.contains(&word.chars().nth(1).unwrap())
        {
            return true;
        }

        return false;
    }

    fn is_short(&self, word: &String) -> bool {
        if self.r1 >= word.len() {
            return self.ends_with_short_syllabe(&word);
        }

        return false;
    }

    fn remove_initial_apostrophe(&self, word: &mut String) {
        if word.starts_with("'") {
            let _ = &word.remove(0);
        }
    }

    fn set_ys(&self, word: &mut String) {
        if word.starts_with("y") {
            word.replace_range(0..1, "Y");
        }

        let mut is_vowel = false;
        let chars: Vec<char> = word.chars().collect();

        for (i, c) in chars.into_iter().enumerate() {
            if is_vowel && c == 'y' {
                word.replace_range(i..i + 1, "Y");
            }

            if VOWELS.contains(&c) {
                is_vowel = true;
            } else {
                is_vowel = false;
            }
        }
    }

    fn find_r1r2(&mut self, word: &mut String) {
        self.r1 = word.len();
        self.r2 = word.len();

        for prefix in R1_BEGININGS.iter() {
            if !word.starts_with(prefix) {
                continue;
            }

            self.r1 = prefix.len();

            let chars: &Vec<char> = &word[self.r1..].chars().collect();
            let mut is_vowel = false;

            for (i, c) in chars.into_iter().enumerate() {
                if VOWELS.contains(&c) {
                    is_vowel = true;
                } else {
                    if is_vowel {
                        self.r2 = self.r1 + i + 1;
                        break;
                    }

                    is_vowel = false;
                }
            }

            return;
        }

        let chars: Vec<char> = word.chars().collect();
        let mut is_vowel = false;
        let mut matches = 0;

        for (i, c) in chars.into_iter().enumerate() {
            if VOWELS.contains(&c) {
                is_vowel = true;
            } else {
                if is_vowel && matches == 0 {
                    matches += 1;
                    self.r1 = i + 1;
                } else if is_vowel && matches == 1 {
                    self.r2 = i + 1;
                    break;
                }

                is_vowel = false;
            }
        }
    }

    fn step_0(&self, word: &mut String) {
        if word.ends_with("'s'") {
            word.truncate(word.len() - 3);
        } else if word.ends_with("'s") {
            word.truncate(word.len() - 2);
        } else if word.ends_with("'") {
            word.truncate(word.len() - 1);
        }
    }

    fn step_1a(&self, word: &mut String) {
        for suffix in STEP_1A_SUFFIXES.iter() {
            if !word.ends_with(suffix) {
                continue;
            }

            if *suffix == "sses" {
                word.truncate(word.len() - 2);
            } else if ["ied", "ies"].contains(&suffix) {
                word.truncate(word.len() - 3);

                if word.len() > 1 {
                    word.insert_str(word.len(), "i");
                } else {
                    word.insert_str(word.len(), "ie");
                }
            } else if *suffix == "s" && word.len() > 2 {
                let chars: &Vec<char> = &word[..word.len() - 2].chars().collect();
                for c in chars.into_iter() {
                    if VOWELS.contains(&c) {
                        word.truncate(word.len() - 1);
                        break;
                    }
                }
            }

            break;
        }
    }

    fn step_1b(&self, word: &mut String) {
        for suffix in STEP_1B_SUFFIXES_1.iter() {
            if !word.ends_with(suffix) {
                continue;
            }

            if word.len() - suffix.len() >= self.r1
                && !["proc", "exc", "succ"].iter().any(|p| word.starts_with(p))
            {
                word.replace_range(word.len() - suffix.len().., "ee");
            }

            return;
        }

        for suffix in STEP_1B_SUFFIXES_2.iter() {
            if !word.ends_with(suffix) {
                continue;
            }

            // special case for 'ing'
            if *suffix == "ing" {
                if word.len() == 5
                    && word.chars().nth(word.len() - 4).unwrap() == 'y'
                    && !VOWELS.contains(&word.chars().nth(word.len() - 5).unwrap())
                {
                    word.replace_range(word.len() - 4.., "ie");
                    return;
                } else if ["inn", "out", "cann", "herr", "earr", "even"]
                    .contains(&&word[..word.len() - 3])
                {
                    return;
                }
            }

            if word[..word.len() - suffix.len()]
                .chars()
                .into_iter()
                .any(|c| VOWELS.contains(&c))
            {
                // delete suffix
                word.truncate(word.len() - suffix.len());

                if ["at", "bl", "iz"].iter().any(|s| word.ends_with(s)) {
                    word.insert(word.len(), 'e');
                } else if DOUBLES.iter().any(|s| word.ends_with(s))
                    && !(word.len() == 3 && ['a', 'e', 'o'].contains(&word.chars().nth(0).unwrap()))
                {
                    word.truncate(word.len() - 1);
                } else if self.is_short(&word) {
                    word.insert(word.len(), 'e');
                }
            }

            break;
        }
    }

    fn step_1c(&self, word: &mut String) {
        if word.len() > 2
            && ['y', 'Y'].contains(&word.chars().nth(word.len() - 1).unwrap())
            && !VOWELS.contains(&word.chars().nth(word.len() - 2).unwrap())
        {
            word.replace_range(word.len() - 1.., "i");
        }
    }

    fn step_2(&self, word: &mut String) {
        for suffix in STEP_2_SUFFIXES.iter() {
            if !word.ends_with(suffix) {
                continue;
            }

            if !(word.len() - suffix.len() >= self.r1) {
                return;
            }

            let repl = self.step_2_suffix_map.get(*suffix).unwrap();
            if word.len() >= 4
                && *suffix == "ogi"
                && word.chars().nth(word.len() - 4).unwrap() == 'l'
            {
                word.replace_range(word.len() - suffix.len().., repl);
            } else if word.len() >= 3
                && *suffix == "li"
                && LI_ENDINGS.contains(&word.chars().nth(word.len() - 3).unwrap())
            {
                word.replace_range(word.len() - suffix.len().., repl);
            } else if !["ogi", "li"].contains(suffix) {
                word.replace_range(word.len() - suffix.len().., repl);
            }

            break;
        }
    }

    fn step_3(&self, word: &mut String) {
        for suffix in STEP_3_SUFFIXES.iter() {
            if !word.ends_with(suffix) {
                continue;
            }

            if !(word.len() - suffix.len() >= self.r1) {
                return;
            }

            let repl = self.step_3_suffix_map.get(*suffix).unwrap();
            if *suffix == "ative" && word.len() - suffix.len() >= self.r2 {
                word.replace_range(word.len() - suffix.len().., repl);
            } else if *suffix != "ative" {
                word.replace_range(word.len() - suffix.len().., repl);
            }

            break;
        }
    }

    fn step_4(&self, word: &mut String) {
        for suffix in STEP_4_SUFFIXES.iter() {
            if !word.ends_with(suffix) {
                continue;
            }

            if !(word.len() - suffix.len() >= self.r2) {
                return;
            }

            if word.len() > 3
                && *suffix == "ion"
                && ['s', 't'].contains(&word.chars().nth(word.len() - 4).unwrap())
            {
                word.truncate(word.len() - suffix.len());
            } else if *suffix != "ion" {
                word.truncate(word.len() - suffix.len());
            }

            break;
        }
    }

    fn step_5(&self, word: &mut String) {
        if word.ends_with("e")
            && ((word.len() - 1 >= self.r2)
                || (word.len() - 1 >= self.r1
                    && !self.ends_with_short_syllabe(&word[..word.len() - 1])))
        {
            word.truncate(word.len() - 1);
        } else if word.ends_with("ll") && word.len() - 1 >= self.r2 {
            word.truncate(word.len() - 1);
        }
    }
}
