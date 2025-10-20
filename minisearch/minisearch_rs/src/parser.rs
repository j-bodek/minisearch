use chumsky::prelude::*;

enum Fuzz {
    Strict(u32),
    Auto,
}

#[derive(Clone, Debug)]
pub struct Term {
    text: String,
    fuzz: u32,
}

#[derive(Clone, Debug)]
pub struct Query {
    terms: Vec<Term>,
    slop: u32,
}

pub fn query<'a>() -> impl Parser<'a, &'a str, Query> {
    let token = any()
        .filter(|c: &char| !char::is_whitespace(*c) && *c != '"' && *c != '~')
        .repeated()
        .at_least(1)
        .collect::<String>();

    // FUZZ = "~" + DIGITS.optional()
    let number = text::digits(10)
        .at_least(1)
        .collect::<String>()
        .map(|s| s.parse::<u32>().unwrap());

    let fuzz = just('~').ignore_then(number.or_not().map(|num| match num {
        Some(v) => Fuzz::Strict(v),
        None => Fuzz::Auto,
    }));

    // SLOP = "~" + DIGITS
    let slop = just('~').ignore_then(number);

    // TERM = TOKEN then FUZZ.optional()
    let term = token.then(fuzz.or_not());
    let ws = text::whitespace().at_least(1);

    // PHRASE = quote then repeated terms seperated by whitespace then quote
    let terms = term
        .separated_by(ws)
        .at_least(1)
        .collect::<Vec<_>>()
        .map(|v| {
            v.iter()
                .map(|val| Term {
                    text: val.0.clone(),
                    fuzz: match &val.1 {
                        Some(x) => match x {
                            Fuzz::Strict(v) => *v,
                            Fuzz::Auto => val.0.len() as u32,
                        },
                        None => 0,
                    },
                })
                .collect()
        });

    let phrase = just('"')
        .ignore_then(terms)
        .then_ignore(just('"'))
        .then(slop);

    // QUERY = (PHRASE then SLOP) or repeated terms seperated by whitespace
    let query = phrase
        .map(|val| Query {
            terms: val.0,
            slop: val.1,
        })
        .or(just('"')
            .or_not()
            .ignore_then(terms)
            .then_ignore(just('"').or_not())
            .map(|terms| Query {
                terms: terms,
                slop: 0,
            }));

    query
}
