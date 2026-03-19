use crate::{
    lexer::lex,
    parser::{parse, parse_term, Statement, Term},
};

/// Parses a full statement from a string literal.
pub fn stmt(input: &'static str) -> Statement {
    parse(lex(input).unwrap()).unwrap()
}

/// Parses a term from a string literal.
pub fn term(input: &'static str) -> Term {
    parse_term(lex(input).unwrap()).unwrap()
}
