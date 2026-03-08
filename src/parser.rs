/* grammar

term        := lambda | application
lambda      := LAMBDA IDENT DOT term
application := atom atom*
atom        := IDENT | LPAREN term RPAREN
*/

use std::fmt::{Debug, Display};

use crate::lexer::{TokenKind, TokenSpan};

#[derive(Debug, PartialEq, Clone)]
pub enum Term {
    Lambda(String, Box<Term>),
    Application(Box<Term>, Box<Term>),
    Var(String),
}

#[derive(Debug, PartialEq, Clone)]
pub enum ParseError {
    UnexpectedToken {
        expected: &'static str,
        found: TokenSpan,
    },
    UnexpectedEof {
        expected: &'static str,
        pos: usize,
    },
}

impl Display for ParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ParseError::UnexpectedToken { expected, found } => {
                write!(
                    f,
                    "unexpected token at {}..{} (expected {expected})",
                    found.start, found.end
                )
            }
            ParseError::UnexpectedEof { expected, pos } => {
                write!(f, "unexpected end of input at {pos} (expected {expected})")
            }
        }
    }
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
    input: Vec<TokenSpan>,
}

impl Parser {
    fn new(input: Vec<TokenSpan>) -> Parser {
        Parser { pos: 0, input }
    }

    fn peek(&self) -> Option<&TokenSpan> {
        if self.pos >= self.input.len() {
            return None;
        }

        self.input.get(self.pos)
    }

    fn advance(&mut self) {
        if self.pos < self.input.len() {
            self.pos += 1;
        }
    }

    fn eof_pos(&self) -> usize {
        self.input.last().map(|token| token.end).unwrap_or(0)
    }

    fn parse_atom(&mut self) -> Result<Term, ParseError> {
        let token = match self.peek() {
            Some(token) => token,
            None => {
                return Err(ParseError::UnexpectedEof {
                    expected: "identifier or '('",
                    pos: self.eof_pos(),
                })
            }
        };

        match &token.kind {
            TokenKind::Ident(name) => {
                let name = name.clone();
                self.advance();
                Ok(Term::Var(name))
            }
            TokenKind::LParen => {
                self.advance();
                let term = self.parse_term()?;
                match self.peek() {
                    Some(TokenSpan {
                        kind: TokenKind::RParen,
                        ..
                    }) => {
                        self.advance();
                        Ok(term)
                    }
                    Some(found) => Err(ParseError::UnexpectedToken {
                        expected: "')'",
                        found: found.clone(),
                    }),
                    None => Err(ParseError::UnexpectedEof {
                        expected: "')'",
                        pos: self.eof_pos(),
                    }),
                }
            }
            _ => Err(ParseError::UnexpectedToken {
                expected: "identifier or '('",
                found: token.clone(),
            }),
        }
    }

    fn is_next_token_atom(&self) -> bool {
        matches!(
            self.peek(),
            Some(TokenSpan {
                kind: TokenKind::LParen,
                ..
            }) | Some(TokenSpan {
                kind: TokenKind::Ident(_),
                ..
            })
        )
    }

    fn parse_application(&mut self) -> Result<Term, ParseError> {
        let mut left = self.parse_atom()?;

        if !self.is_next_token_atom() {
            return Ok(left);
        }

        while self.is_next_token_atom() {
            let right = self.parse_atom()?;
            left = Term::Application(Box::new(left), Box::new(right));
        }

        Ok(left)
    }

    fn parse_lambda(&mut self) -> Result<Term, ParseError> {
        match self.peek() {
            Some(TokenSpan {
                kind: TokenKind::Lambda,
                ..
            }) => self.advance(),
            Some(found) => {
                return Err(ParseError::UnexpectedToken {
                    expected: "lambda",
                    found: found.clone(),
                })
            }
            None => {
                return Err(ParseError::UnexpectedEof {
                    expected: "lambda",
                    pos: self.eof_pos(),
                })
            }
        }

        let identifier = match self.peek() {
            Some(TokenSpan {
                kind: TokenKind::Ident(name),
                ..
            }) => {
                let name = name.clone();
                self.advance();
                name
            }
            Some(found) => {
                return Err(ParseError::UnexpectedToken {
                    expected: "identifier",
                    found: found.clone(),
                })
            }
            None => {
                return Err(ParseError::UnexpectedEof {
                    expected: "identifier",
                    pos: self.eof_pos(),
                })
            }
        };

        match self.peek() {
            Some(TokenSpan {
                kind: TokenKind::Dot,
                ..
            }) => self.advance(),
            Some(found) => {
                return Err(ParseError::UnexpectedToken {
                    expected: "'.'",
                    found: found.clone(),
                })
            }
            None => {
                return Err(ParseError::UnexpectedEof {
                    expected: "'.'",
                    pos: self.eof_pos(),
                })
            }
        }

        let body = self.parse_term()?;

        Ok(Term::Lambda(identifier, Box::new(body)))
    }

    fn parse_term(&mut self) -> Result<Term, ParseError> {
        match self.peek() {
            Some(TokenSpan {
                kind: TokenKind::Lambda,
                ..
            }) => self.parse_lambda(),
            _ => self.parse_application(),
        }
    }
}

pub fn parse(input: Vec<TokenSpan>) -> Result<Term, ParseError> {
    let mut parser = Parser::new(input);
    let term = parser.parse_term()?;

    match parser.peek() {
        Some(TokenSpan {
            kind: TokenKind::EOF,
            ..
        }) => Ok(term),
        Some(found) => Err(ParseError::UnexpectedToken {
            expected: "end of input",
            found: found.clone(),
        }),
        None => Ok(term),
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        lexer::lex,
        parser::{parse, Term},
    };

    fn lex_and_parse(input: &str) -> Term {
        let tokens = lex(input).unwrap();
        parse(tokens).unwrap()
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
