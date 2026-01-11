use chumsky::prelude::*;
use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;
use std::str::FromStr;

enum Fuzz {
    Strict(u8),
    Auto,
}

#[derive(Clone, Debug)]
pub struct Term<'a> {
    pub text: &'a str,
    pub fuzz: u8,
}

#[derive(Clone, Debug)]
pub struct Query<'a> {
    pub terms: Vec<Term<'a>>,
    pub slop: u8,
}

impl<'a> Query<'a> {
    pub fn parse(query: &'a mut str) -> Result<Query<'a>, PyErr> {
        query.make_ascii_lowercase();
        let result = Self::parser().parse(query);
        if result.has_errors() {
            let errors = result
                .errors()
                .map(|e| format!("{:?}", e))
                .collect::<Vec<String>>()
                .join("\n");

            return Err(PyValueError::new_err(format!(
                "Following query is invalid: '{}'\n, {}",
                query, errors
            )));
        }

        match result.into_output() {
            Some(res) => Ok(res),
            None => Err(PyValueError::new_err(
                "Failed to parse query, the output is empty",
            )),
        }
    }

    fn map_auto_fuzz(len: usize) -> u8 {
        match len {
            _ if len <= 2 => 0,
            _ if len <= 5 => 1,
            _ => 2,
        }
    }

    fn parser() -> impl Parser<'a, &'a str, Query<'a>, extra::Err<Rich<'a, char>>> {
        // TOKEN = any string that do not contain whitespaces, double quotes or tildas
        let token = any()
            .filter(|c: &char| !char::is_whitespace(*c) && *c != '"' && *c != '~')
            .repeated()
            .at_least(1)
            .to_slice();

        let number = text::digits(10)
            .at_least(1)
            .to_slice()
            .map(|s| u8::from_str(s).unwrap());

        // FUZZ = "~" + optional number
        let fuzz = just('~')
            .ignore_then(number.or_not().map(|num| match num {
                Some(v) => Fuzz::Strict(v),
                None => Fuzz::Auto,
            }))
            .validate(|x, e, emitter| {
                match x {
                    Fuzz::Strict(v) => {
                        if v > 2 {
                            emitter.emit(Rich::custom(
                                e.span(),
                                format!("Fuzziness must be less or equal to 2, but it is {}.", v),
                            ))
                        }
                    }
                    _ => (),
                };
                x
            });

        // SLOP = "~" + DIGITS
        let slop = just('~').ignore_then(number);

        // TERM = TOKEN then FUZZ.optional()
        let term = token.then(fuzz.or_not());

        // PHRASE = quote then repeated terms seperated by whitespace then quote
        let ws = text::whitespace().at_least(1);
        let terms = term
            .separated_by(ws)
            .at_least(1)
            .collect::<Vec<_>>()
            .map(|v| {
                v.into_iter()
                    .map(|val: (&str, Option<Fuzz>)| Term {
                        fuzz: match &val.1 {
                            Some(x) => match x {
                                Fuzz::Strict(v) => *v,
                                Fuzz::Auto => Self::map_auto_fuzz(val.0.len()),
                            },
                            None => 0,
                        },
                        text: val.0,
                    })
                    .collect()
            });

        let phrase = just('"')
            .ignore_then(terms)
            .then_ignore(just('"'))
            .then(slop.or_not());

        // QUERY = (PHRASE then SLOP) or repeated terms seperated by whitespace
        let query = text::whitespace()
            .ignore_then(phrase)
            .map(|val| Query {
                terms: val.0,
                slop: match val.1 {
                    Some(v) => v,
                    _ => 0,
                },
            })
            .or(terms.map(|terms| Query {
                terms: terms,
                slop: 0,
            }))
            .then_ignore(text::whitespace())
            .then_ignore(end());

        query
    }
}
