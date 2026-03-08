use lc::{
    lexer::lex,
    parser::{Term, parse}, repl,
};

fn lex_and_parse(input: &str) -> Term {
    let tokens = lex(input);
    parse(tokens)
}

fn main() {
    repl::repl()
}
