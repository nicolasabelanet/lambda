use lc::{
    lexer::lex,
    parser::{Term, parse},
};

fn lex_and_parse(input: &str) -> Term {
    let tokens = lex(input);
    parse(tokens)
}

fn main() {
    println!("{}", lex_and_parse("f (\\x.x)"));
}
