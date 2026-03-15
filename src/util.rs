use crate::{
    lexer::lex,
    parser::{Statement, Term, parse, parse_term},
};

pub fn stmt(input: &'static str) -> Statement {
    parse(lex(input).unwrap()).unwrap()
}

pub fn term(input: &'static str) -> Term {
    parse_term(lex(input).unwrap()).unwrap()
}
