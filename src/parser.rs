/* grammar

term        := let_expr | lambda | application
lambda      := LAMBDA IDENT DOT term
application := atom atom*
atom        := IDENT | LPAREN term RPAREN
let_expr    := LET IDENT EQUAL term IN term
*/

use std::fmt::{Debug, Display};

use crate::lexer::{Token, TokenKind, lex};

#[derive(Debug, PartialEq, Clone)]
pub enum Term {
    Lambda(String, Option<Type>, Box<Term>),
    Application(Box<Term>, Box<Term>),
    Var(String),
    Let {
        name: String,
        value: Box<Term>,
        body: Box<Term>,
    },
}

#[derive(Debug, PartialEq, Clone)]
pub enum Type {
    Var(String),
    Arrow(Box<Type>, Box<Type>),
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

impl Display for Type {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            Type::Var(name) => write!(f, "{name}"),
            Type::Arrow(left, right) => {
                // Parenthesize left if it's also an arrow.
                match **left {
                    Type::Arrow(_, _) => write!(f, "({left}) -> {right}"),
                    _ => write!(f, "{left} -> {right}"),
                }
            }
        }
    }
}

impl Display for Term {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            Term::Var(name) => write!(f, "{name}"),
            Term::Lambda(name, ty_monad, body) => {
                let string_body = body.to_string();

                let mut string_ty = "".to_string();
                if let Some(ty) = ty_monad {
                    string_ty = format!(": {ty}");
                }
                write!(f, "\\{name}{string_ty}.{}", string_body)
            }
            Term::Let { name, value, body } => {
                write!(f, "let {name} = {value} in {body}")
            }
            Term::Application(left, right) => {
                let mut left_string = left.to_string();

                if let Term::Lambda(_, _, _) = **left {
                    left_string = format!("({})", left_string);
                }

                let mut right_string = right.to_string();

                if let Term::Lambda(_, _, _) | Term::Application(_, _) = **right {
                    right_string = format!("({})", right_string);
                }
                write!(f, "{left_string} {right_string}")
            }
        }
    }
}

pub fn stmt(input: &str) -> Statement {
    parse(lex(input).unwrap()).unwrap()
}

pub fn term(input: &str) -> Term {
    parse_term(lex(input).unwrap()).unwrap()
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
        self.input.get(self.pos)
    }

    fn peek_is(&self, kind: &TokenKind) -> bool {
        matches!(self.peek(), Some(Token { kind: k, .. }) if k == kind)
    }

    fn advance(&mut self) {
        if self.pos < self.input.len() {
            self.pos += 1;
        }
    }

    fn eof_pos(&self) -> usize {
        self.input.last().map(|token| token.span.end).unwrap_or(0)
    }

    fn expect_or_unexpected(
        &mut self,
        kind: TokenKind,
        expected: &'static str,
    ) -> Result<(), ParseError> {
        match self.peek() {
            Some(token) if token.kind == kind => {
                self.advance();
                Ok(())
            }
            Some(found) => Err(ParseError::UnexpectedToken {
                expected,
                found: found.clone(),
            }),
            None => Err(ParseError::UnexpectedEof {
                expected,
                pos: self.eof_pos(),
            }),
        }
    }

    fn expect(&mut self, kind: TokenKind, expected: &'static str) -> Result<(), ParseError> {
        match self.peek() {
            Some(token) if token.kind == kind => {
                self.advance();
                Ok(())
            }
            Some(found) => Err(ParseError::MissingToken {
                expected,
                pos: found.span.start,
            }),
            None => Err(ParseError::MissingToken {
                expected,
                pos: self.eof_pos(),
            }),
        }
    }

    fn parse_expr_with_parens<T>(
        &mut self,
        inner_parser: fn(&mut Parser) -> Result<T, ParseError>,
        inner_type: &'static str,
        ident_to_value: fn(String) -> T,
    ) -> Result<T, ParseError> {
        let token = match self.peek() {
            Some(token) => token,
            None => {
                return Err(ParseError::MissingToken {
                    expected: inner_type,
                    pos: self.eof_pos(),
                });
            }
        };

        match &token.kind {
            TokenKind::Ident(_) => Ok(ident_to_value(self.parse_identifier()?)),
            TokenKind::LParen => {
                self.advance();
                let inner = inner_parser(self)?;
                self.expect(TokenKind::RParen, "')'")?;
                Ok(inner)
            }
            TokenKind::EOF => Err(ParseError::MissingToken {
                expected: inner_type,
                pos: token.span.start,
            }),
            _ => Err(ParseError::UnexpectedToken {
                expected: inner_type,
                found: token.clone(),
            }),
        }
    }

    fn parse_type_atom(&mut self) -> Result<Type, ParseError> {
        self.parse_expr_with_parens(Parser::parse_type, "type", Type::Var)
    }

    fn parse_atom(&mut self) -> Result<Term, ParseError> {
        self.parse_expr_with_parens(Parser::parse_term, "term", Term::Var)
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

    fn parse_identifier(&mut self) -> Result<String, ParseError> {
        match self.peek() {
            Some(Token {
                kind: TokenKind::Ident(name),
                ..
            }) => {
                let name = name.clone();
                self.advance();
                Ok(name)
            }
            Some(found) => Err(ParseError::MissingToken {
                expected: "identifier",
                pos: found.span.start,
            }),
            None => Err(ParseError::MissingToken {
                expected: "identifier",
                pos: self.eof_pos(),
            }),
        }
    }

    fn parse_lambda(&mut self) -> Result<Term, ParseError> {
        self.expect_or_unexpected(TokenKind::Lambda, "lambda")?;

        let identifier = self.parse_identifier()?;

        let ty = match self.peek() {
            Some(Token {
                kind: TokenKind::Colon,
                ..
            }) => {
                self.advance();
                Some(self.parse_type()?)
            }
            _ => None,
        };

        self.expect(TokenKind::Dot, "'.'")?;

        let body = self.parse_term()?;

        Ok(Term::Lambda(identifier, ty, Box::new(body)))
    }

    fn parse_type(&mut self) -> Result<Type, ParseError> {
        let left = self.parse_type_atom()?;

        if self.peek_is(&TokenKind::Arrow) {
            self.advance();
            let right = self.parse_type()?;
            Ok(Type::Arrow(Box::new(left), Box::new(right)))
        } else {
            Ok(left)
        }
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
        self.expect_or_unexpected(TokenKind::In, "in")?;
        self.parse_term()
    }

    fn parse_let_head(&mut self) -> Result<(String, Term), ParseError> {
        self.expect_or_unexpected(TokenKind::Let, "let")?;

        let name = self.parse_identifier()?;

        self.expect_or_unexpected(TokenKind::Equals, "'='")?;

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
    let mut parser = Parser::new(input);
    let statement = parser.parse_statement()?;

    match parser.peek() {
        Some(Token {
            kind: TokenKind::EOF,
            ..
        }) => Ok(statement),
        Some(found) => Err(ParseError::UnexpectedToken {
            expected: "end of input",
            found: found.clone(),
        }),
        None => Ok(statement),
    }
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
        lexer::{Span, Token, lex},
        parser::{ParseError, Statement, Term, parse, parse_term},
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
            None,
            Box::new(Term::Application(
                Box::new(Term::Var("x".into())),
                Box::new(Term::Var("y".into())),
            )),
        ));
        assert_roundtrip(Term::Application(
            Box::new(Term::Lambda(
                "x".into(),
                None,
                Box::new(Term::Var("x".into())),
            )),
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
                Box::new(Term::Lambda(
                    "x".into(),
                    None,
                    Box::new(Term::Var("x".into())),
                )),
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
                None,
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
                Box::new(Term::Lambda(
                    "x".into(),
                    None,
                    Box::new(Term::Var("x".into())),
                )),
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
            Term::Lambda("x".into(), None, Box::new(Term::Var("x".into())))
        );

        assert_eq!(
            lex_and_parse("(\\x.x) y"),
            Term::Application(
                Box::new(Term::Lambda(
                    "x".into(),
                    None,
                    Box::new(Term::Var("x".into())),
                )),
                Box::new(Term::Var("y".into()))
            )
        );
        assert_eq!(
            lex_and_parse("\\x.x"),
            Term::Lambda("x".into(), None, Box::new(Term::Var("x".into())))
        );
        assert_eq!(
            lex_and_parse("f (\\x.x)"),
            Term::Application(
                Box::new(Term::Var("f".into())),
                Box::new(Term::Lambda(
                    "x".into(),
                    None,
                    Box::new(Term::Var("x".into())),
                )),
            )
        );
        assert_eq!(lex_and_parse("((x))"), Term::Var("x".into()));
        assert_eq!(
            lex_and_parse("\\x.x y"),
            Term::Lambda(
                "x".into(),
                None,
                Box::new(Term::Application(
                    Box::new(Term::Var("x".into())),
                    Box::new(Term::Var("y".into())),
                )),
            )
        );
        assert_eq!(
            lex_and_parse("let id = \\x.x in id y"),
            Term::Let {
                name: "id".into(),
                value: Box::new(Term::Lambda(
                    "x".into(),
                    None,
                    Box::new(Term::Var("x".into())),
                )),
                body: Box::new(Term::Application(
                    Box::new(Term::Var("id".into())),
                    Box::new(Term::Var("y".into())),
                )),
            }
        );
    }

    #[test]
    fn test_parse_statement_global_let() {
        let tokens = lex("let x = y").unwrap();
        let statement = parse(tokens).unwrap();
        assert!(matches!(
            statement,
            Statement::Let(name, Term::Var(value)) if name == "x" && value == "y"
        ));
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
