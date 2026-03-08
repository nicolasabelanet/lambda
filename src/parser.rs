/* grammar

term        := lambda | application
lambda      := LAMBDA IDENT DOT term
application := atom atom*
atom        := IDENT | LPAREN term RPAREN
*/

use std::fmt::{Debug, Display};

use crate::lexer::Token;

#[derive(Debug, PartialEq)]
pub enum Term {
    Lambda(String, Box<Term>),
    Application(Box<Term>, Box<Term>),
    Var(String),
}

impl Display for Term {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            Term::Var(name) => write!(f, "{name}"),
            Term::Lambda(name, body) => {
                let string_body = body.to_string();
                write!(f, "\\{name}.{}", string_body)
            }
            Term::Application(left, right) => {
                let mut left_string = left.to_string();

                if let Term::Lambda(_, _) = **left {
                    left_string = format!("({})", left_string);
                }

                let mut right_string = right.to_string();

                if let Term::Lambda(_, _) | Term::Application(_, _) = **right {
                    right_string = format!("({})", right_string);
                }
                write!(f, "{left_string} {right_string}")
            }
        }
    }
}

struct Parser {
    pos: usize,
    input: Vec<Token>,
}

impl Parser {
    fn new(input: Vec<Token>) -> Parser {
        Parser { pos: 0, input }
    }

    fn peek(&self) -> Option<Token> {
        if self.pos >= self.input.len() {
            return None;
        }

        let token = self.input.get(self.pos).cloned();
        dbg!(&token);
        token
    }

    fn advance(&mut self) {
        if self.pos < self.input.len() {
            self.pos += 1;
        }
    }

    fn parse_atom(&mut self) -> Term {
        println!("Parsing atom");
        match self.peek() {
            Some(Token::Ident(name)) => {
                self.advance();
                Term::Var(name)
            }
            Some(Token::LParen) => {
                self.advance();
                let term = self.parse_term();
                let rparen = self.peek();
                assert!(matches!(rparen, Some(Token::RParen)));
                self.advance();
                term
            }
            _ => panic!(),
        }
    }

    fn is_next_token_atom(&self) -> bool {
        matches!(self.peek(), Some(Token::LParen) | Some(Token::Ident(_)))
    }

    fn parse_application(&mut self) -> Term {
        let mut left = self.parse_atom();

        if !self.is_next_token_atom() {
            return left;
        }

        let mut right = self.parse_atom();

        let mut app = Term::Application(Box::new(left), Box::new(right));

        while self.is_next_token_atom() {
            left = app;
            right = self.parse_atom();
            app = Term::Application(Box::new(left), Box::new(right))
        }

        app
    }

    fn parse_lambda(&mut self) -> Term {
        let lambda = self.peek();
        assert!(matches!(lambda, Some(Token::Lambda)));
        self.advance();

        let identifier = self.peek().unwrap();
        self.advance();

        let dot = self.peek();
        assert!(matches!(dot, Some(Token::Dot)));
        self.advance();

        let body = self.parse_term();

        if let Token::Ident(name) = identifier {
            return Term::Lambda(name, Box::new(body));
        };
        panic!()
    }

    fn parse_term(&mut self) -> Term {
        match self.peek() {
            Some(Token::Lambda) => self.parse_lambda(),
            _ => self.parse_application(),
        }
    }
}

pub fn parse(input: Vec<Token>) -> Term {
    let mut parser = Parser::new(input);
    parser.parse_term()
}

#[cfg(test)]
mod tests {
    use crate::{
        lexer::lex,
        parser::{Term, parse},
    };

    fn lex_and_parse(input: &str) -> Term {
        let tokens = lex(input);
        parse(tokens)
    }

    fn assert_roundtrip(term: Term) {
        let rountrip = lex_and_parse(&term.to_string());
        assert_eq!(term, rountrip);
    }

    #[test]
    fn test_roundtrip() {
        assert_roundtrip(Term::Var("x".into()));
        assert_roundtrip(Term::Lambda(
            "x".into(),
            Box::new(Term::Application(
                Box::new(Term::Var("x".into())),
                Box::new(Term::Var("y".into())),
            )),
        ));
        assert_roundtrip(Term::Application(
            Box::new(Term::Lambda("x".into(), Box::new(Term::Var("x".into())))),
            Box::new(Term::Var("y".into())),
        ));

        assert_roundtrip(Term::Application(
            Box::new(Term::Application(
                Box::new(Term::Var("f".into())),
                Box::new(Term::Var("x".into())),
            )),
            Box::new(Term::Var("y".into())),
        ));
        assert_roundtrip(Term::Application(
            Box::new(Term::Var("f".into())),
            Box::new(Term::Application(
                Box::new(Term::Var("x".into())),
                Box::new(Term::Var("y".into())),
            )),
        ));
    }

    #[test]
    fn test_pretty_printer() {
        assert_eq!(
            Term::Application(
                Box::new(Term::Lambda("x".into(), Box::new(Term::Var("x".into())))),
                Box::new(Term::Var("y".into()))
            )
            .to_string(),
            "(\\x.x) y"
        );

        assert_eq!(
            Term::Application(
                Box::new(Term::Var("f".into())),
                Box::new(Term::Application(
                    Box::new(Term::Var("x".into())),
                    Box::new(Term::Var("y".into()))
                )),
            )
            .to_string(),
            "f (x y)"
        );
        assert_eq!(
            Term::Lambda(
                "x".into(),
                Box::new(Term::Application(
                    Box::new(Term::Var("x".into())),
                    Box::new(Term::Var("y".into())),
                )),
            )
            .to_string(),
            "\\x.x y"
        );
        assert_eq!(
            Term::Application(
                Box::new(Term::Var("f".into())),
                Box::new(Term::Lambda("x".into(), Box::new(Term::Var("x".into())))),
            )
            .to_string(),
            "f (\\x.x)"
        );
        assert_eq!(
            Term::Application(
                Box::new(Term::Application(
                    Box::new(Term::Var("f".into())),
                    Box::new(Term::Var("x".into())),
                )),
                Box::new(Term::Var("y".into())),
            )
            .to_string(),
            "f x y"
        );
    }

    #[test]
    fn test_parser() {
        assert_eq!(lex_and_parse("x"), Term::Var("x".into()));
        assert_eq!(
            lex_and_parse("f x"),
            Term::Application(
                Box::new(Term::Var("f".into())),
                Box::new(Term::Var("x".into()))
            )
        );
        assert_eq!(
            lex_and_parse("f x y"),
            Term::Application(
                Box::new(Term::Application(
                    Box::new(Term::Var("f".into())),
                    Box::new(Term::Var("x".into()))
                )),
                Box::new(Term::Var("y".into()))
            )
        );
        assert_eq!(lex_and_parse("(x)"), Term::Var("x".into()));
        assert_eq!(
            lex_and_parse("(\\x.x)"),
            Term::Lambda("x".into(), Box::new(Term::Var("x".into())))
        );

        assert_eq!(
            lex_and_parse("(\\x.x) y"),
            Term::Application(
                Box::new(Term::Lambda("x".into(), Box::new(Term::Var("x".into())))),
                Box::new(Term::Var("y".into()))
            )
        );
        assert_eq!(
            lex_and_parse("\\x.x"),
            Term::Lambda("x".into(), Box::new(Term::Var("x".into())))
        );
        assert_eq!(
            lex_and_parse("f (\\x.x)"),
            Term::Application(
                Box::new(Term::Var("f".into())),
                Box::new(Term::Lambda("x".into(), Box::new(Term::Var("x".into())))),
            )
        );
        assert_eq!(lex_and_parse("((x))"), Term::Var("x".into()));
        assert_eq!(
            lex_and_parse("\\x.x y"),
            Term::Lambda(
                "x".into(),
                Box::new(Term::Application(
                    Box::new(Term::Var("x".into())),
                    Box::new(Term::Var("y".into())),
                )),
            )
        );
    }
}
