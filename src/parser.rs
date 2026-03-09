/* grammar

term        := let_expr | lambda | application
lambda      := LAMBDA IDENT DOT term
application := atom atom*
atom        := IDENT | LPAREN term RPAREN
let_expr    := LET IDENT EQUAL term IN term
*/

use std::fmt::{Debug, Display};

use crate::lexer::{Token, TokenKind};

#[derive(Debug, PartialEq, Clone)]
pub enum Term {
    Lambda(String, Box<Term>),
    Application(Box<Term>, Box<Term>),
    Var(String),
    Let {
        name: String,
        value: Box<Term>,
        body: Box<Term>,
    },
}

#[derive(Debug, PartialEq, Clone)]
pub enum Statement {
    Let(String, Term),
    Expr(Term),
}

#[derive(Debug, PartialEq, Clone)]
pub enum ParseError {
    UnexpectedToken {
        expected: &'static str,
        found: Token,
    },
    UnexpectedEof {
        expected: &'static str,
        pos: usize,
    },
    MissingToken {
        expected: &'static str,
        pos: usize,
    },
}

impl Display for ParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ParseError::UnexpectedToken { expected, found: _ } => {
                write!(f, "unexpected token (expected {expected})")
            }
            ParseError::UnexpectedEof { expected, pos: _ } => {
                write!(f, "unexpected end of input (expected {expected})")
            }
            ParseError::MissingToken { expected, pos: _ } => {
                write!(f, "expected {expected}")
            }
        }
    }
}

impl ParseError {
    pub fn span(&self) -> crate::lexer::Span {
        match self {
            ParseError::UnexpectedToken { found, .. } => crate::lexer::Span {
                start: found.span.start,
                end: found.span.end,
            },
            ParseError::UnexpectedEof { pos, .. } => crate::lexer::Span {
                start: *pos,
                end: *pos,
            },
            ParseError::MissingToken { pos, .. } => crate::lexer::Span {
                start: *pos,
                end: *pos,
            },
        }
    }

    pub fn message(&self) -> String {
        match self {
            ParseError::UnexpectedToken { expected, .. } => {
                format!("expected {expected}")
            }
            ParseError::UnexpectedEof { expected, .. } => {
                format!("unexpected end of input (expected {expected})")
            }
            ParseError::MissingToken { expected, .. } => {
                format!("expected {expected}")
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
            Term::Let { name, value, body } => {
                write!(f, "let {name} = {value} in {body}")
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

    fn peek(&self) -> Option<&Token> {
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
        self.input.last().map(|token| token.span.end).unwrap_or(0)
    }

    fn parse_atom(&mut self) -> Result<Term, ParseError> {
        let token = match self.peek() {
            Some(token) => token,
            None => {
                return Err(ParseError::MissingToken {
                    expected: "term",
                    pos: self.eof_pos(),
                });
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
                    Some(Token {
                        kind: TokenKind::RParen,
                        ..
                    }) => {
                        self.advance();
                        Ok(term)
                    }
                    Some(found) => Err(ParseError::MissingToken {
                        expected: "')'",
                        pos: found.span.start,
                    }),
                    None => Err(ParseError::MissingToken {
                        expected: "')'",
                        pos: self.eof_pos(),
                    }),
                }
            }
            TokenKind::EOF => Err(ParseError::MissingToken {
                expected: "term",
                pos: token.span.start,
            }),
            _ => Err(ParseError::UnexpectedToken {
                expected: "term",
                found: token.clone(),
            }),
        }
    }

    fn is_next_token_atom(&self) -> bool {
        matches!(
            self.peek(),
            Some(Token {
                kind: TokenKind::LParen,
                ..
            }) | Some(Token {
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
            Some(Token {
                kind: TokenKind::Lambda,
                ..
            }) => self.advance(),
            Some(found) => {
                return Err(ParseError::UnexpectedToken {
                    expected: "lambda",
                    found: found.clone(),
                });
            }
            None => {
                return Err(ParseError::UnexpectedEof {
                    expected: "lambda",
                    pos: self.eof_pos(),
                });
            }
        }

        let identifier = match self.peek() {
            Some(Token {
                kind: TokenKind::Ident(name),
                ..
            }) => {
                let name = name.clone();
                self.advance();
                name
            }
            Some(found) => {
                return Err(ParseError::MissingToken {
                    expected: "identifier",
                    pos: found.span.start,
                });
            }
            None => {
                return Err(ParseError::MissingToken {
                    expected: "identifier",
                    pos: self.eof_pos(),
                });
            }
        };

        match self.peek() {
            Some(Token {
                kind: TokenKind::Dot,
                ..
            }) => self.advance(),
            Some(found) => {
                return Err(ParseError::MissingToken {
                    expected: "'.'",
                    pos: found.span.start,
                });
            }
            None => {
                return Err(ParseError::MissingToken {
                    expected: "'.'",
                    pos: self.eof_pos(),
                });
            }
        }

        let body = self.parse_term()?;

        Ok(Term::Lambda(identifier, Box::new(body)))
    }

    fn parse_term(&mut self) -> Result<Term, ParseError> {
        match self.peek() {
            Some(Token {
                kind: TokenKind::Lambda,
                ..
            }) => self.parse_lambda(),
            Some(Token {
                kind: TokenKind::Let,
                ..
            }) => {
                let (name, value) = self.parse_let_head()?;
                let body = self.parse_scoped_let_tail()?;

                Ok(Term::Let {
                    name,
                    value: Box::new(value),
                    body: Box::new(body),
                })
            }
            _ => self.parse_application(),
        }
    }

    fn parse_scoped_let_tail(&mut self) -> Result<Term, ParseError> {
        match self.peek() {
            Some(Token {
                kind: TokenKind::In,
                ..
            }) => self.advance(),
            Some(found) => {
                return Err(ParseError::UnexpectedToken {
                    expected: "in",
                    found: found.clone(),
                });
            }
            None => {
                return Err(ParseError::UnexpectedEof {
                    expected: "in",
                    pos: self.eof_pos(),
                });
            }
        }

        self.parse_term()
    }

    fn parse_let_head(&mut self) -> Result<(String, Term), ParseError> {
        match self.peek() {
            Some(Token {
                kind: TokenKind::Let,
                ..
            }) => self.advance(),

            Some(found) => {
                return Err(ParseError::UnexpectedToken {
                    expected: "let",
                    found: found.clone(),
                });
            }
            None => {
                return Err(ParseError::UnexpectedEof {
                    expected: "let",
                    pos: self.eof_pos(),
                });
            }
        }

        let name = match self.peek() {
            Some(Token {
                kind: TokenKind::Ident(name),
                ..
            }) => {
                let name = name.clone();
                self.advance();
                name
            }

            Some(found) => {
                return Err(ParseError::MissingToken {
                    expected: "identifier",
                    pos: found.span.start,
                });
            }
            None => {
                return Err(ParseError::MissingToken {
                    expected: "identifier",
                    pos: self.eof_pos(),
                });
            }
        };

        match self.peek() {
            Some(Token {
                kind: TokenKind::Equals,
                ..
            }) => self.advance(),

            Some(found) => {
                return Err(ParseError::MissingToken {
                    expected: "'='",
                    pos: found.span.start,
                });
            }
            None => {
                return Err(ParseError::MissingToken {
                    expected: "'='",
                    pos: self.eof_pos(),
                });
            }
        }

        let value = self.parse_term()?;

        Ok((name, value))
    }

    fn parse_statement(&mut self) -> Result<Statement, ParseError> {
        match self.peek() {
            Some(Token {
                kind: TokenKind::Let,
                ..
            }) => {
                let (name, value) = self.parse_let_head()?;

                let body = match self.peek() {
                    Some(Token {
                        kind: TokenKind::In,
                        ..
                    }) => self.parse_scoped_let_tail()?,
                    Some(Token {
                        kind: TokenKind::EOF,
                        ..
                    }) => return Ok(Statement::Let(name.clone(), value)),
                    Some(found) => {
                        return Err(ParseError::MissingToken {
                            expected: "in",
                            pos: found.span.start,
                        });
                    }
                    None => return Ok(Statement::Let(name.clone(), value)),
                };

                Ok(Statement::Expr(Term::Let {
                    name,
                    value: Box::new(value),
                    body: Box::new(body),
                }))
            }
            _ => Ok(Statement::Expr(self.parse_term()?)),
        }
    }
}

pub fn parse(input: Vec<Token>) -> Result<Statement, ParseError> {
    Parser::new(input).parse_statement()
}

pub fn parse_term(input: Vec<Token>) -> Result<Term, ParseError> {
    let mut parser = Parser::new(input);
    let term = parser.parse_term()?;

    match parser.peek() {
        Some(Token {
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
        lexer::{lex, Span, Token},
        parser::{parse_term, ParseError, Term},
    };

    fn lex_and_parse(input: &str) -> Term {
        let tokens = lex(input).unwrap();
        parse_term(tokens).unwrap()
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

    #[test]
    fn test_parse_errors_missing_dot() {
        let tokens = lex("\\x").unwrap();
        let err = parse_term(tokens).unwrap_err();

        assert!(matches!(
            err,
            ParseError::MissingToken {
                expected: "'.'",
                pos: 2usize
            }
        ))
    }

    #[test]
    fn test_parse_errors_missing_rparen() {
        let tokens = lex("(x").unwrap();
        let err = parse_term(tokens).unwrap_err();
        assert!(matches!(
            err,
            ParseError::MissingToken {
                expected: "')'",
                pos: 2usize
            }
        ));
    }

    #[test]
    fn test_parse_errors_missing_term() {
        let tokens = lex("\\x.(").unwrap();
        let err = parse_term(tokens).unwrap_err();
        assert!(matches!(
            err,
            ParseError::MissingToken {
                expected: "term",
                pos: 4usize
            }
        ));
    }

    #[test]
    fn test_parse_errors_trailing_tokens() {
        let tokens = lex("x)").unwrap();
        let err = parse_term(tokens).unwrap_err();

        assert!(matches!(
            err,
            ParseError::UnexpectedToken {
                expected: "end of input",
                found: Token {
                    span: Span {
                        start: 1usize,
                        end: 2usize,
                    },
                    ..
                }
            }
        ));
    }
}
