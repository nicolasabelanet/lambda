/* grammar

term        := let_expr | lambda | application
lambda      := LAMBDA IDENT DOT term
application := atom atom*
atom        := IDENT | LPAREN term RPAREN
let_expr    := LET IDENT EQUAL term IN term
*/

use std::fmt::{Debug, Display};

use crate::{
    lexer::{Span, Token, TokenKind},
    typing::Type,
};

#[derive(Debug, Clone)]
pub enum Term {
    Lambda(String, Option<Type>, Box<Term>, Span),
    Application(Box<Term>, Box<Term>, Span),
    Var(String, Span),
    Let {
        name: String,
        value: Box<Term>,
        body: Box<Term>,
        span: Span,
    },
}

#[derive(Debug, Clone)]
pub enum Statement {
    Let(String, Term, Span),
    Expr(Term, Span),
}

impl PartialEq for Term {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Term::Var(name, _), Term::Var(other, _)) => name == other,
            (
                Term::Lambda(param, ty, body, _),
                Term::Lambda(other_param, other_ty, other_body, _),
            ) => param == other_param && ty == other_ty && body == other_body,
            (Term::Application(left, right, _), Term::Application(other_left, other_right, _)) => {
                left == other_left && right == other_right
            }
            (
                Term::Let {
                    name, value, body, ..
                },
                Term::Let {
                    name: other_name,
                    value: other_value,
                    body: other_body,
                    ..
                },
            ) => name == other_name && value == other_value && body == other_body,
            _ => false,
        }
    }
}

impl PartialEq for Statement {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Statement::Let(name, term, _), Statement::Let(other_name, other_term, _)) => {
                name == other_name && term == other_term
            }
            (Statement::Expr(term, _), Statement::Expr(other_term, _)) => term == other_term,
            _ => false,
        }
    }
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
            Term::Var(name, _) => write!(f, "{name}"),
            Term::Lambda(name, _, body, _) => {
                let string_body = body.to_string();
                write!(f, "\\{name}.{}", string_body)
            }
            Term::Let {
                name, value, body, ..
            } => {
                write!(f, "let {name} = {value} in {body}")
            }
            Term::Application(left, right, _) => {
                let mut left_string = left.to_string();

                if let Term::Lambda(_, _, _, _) = **left {
                    left_string = format!("({})", left_string);
                }

                let mut right_string = right.to_string();

                if let Term::Lambda(_, _, _, _) | Term::Application(_, _, _) = **right {
                    right_string = format!("({})", right_string);
                }
                write!(f, "{left_string} {right_string}")
            }
        }
    }
}

impl Term {
    pub fn span(&self) -> Span {
        match self {
            Term::Var(_, span) => span.clone(),
            Term::Lambda(_, _, _, span) => span.clone(),
            Term::Application(_, _, span) => span.clone(),
            Term::Let { span, .. } => span.clone(),
        }
    }

    pub fn with_span(self, span: Span) -> Term {
        match self {
            Term::Var(name, _) => Term::Var(name, span),
            Term::Lambda(param, ty, body, _) => Term::Lambda(param, ty, body, span),
            Term::Application(left, right, _) => Term::Application(left, right, span),
            Term::Let {
                name, value, body, ..
            } => Term::Let {
                name,
                value,
                body,
                span,
            },
        }
    }
}

impl Statement {
    pub fn span(&self) -> Span {
        match self {
            Statement::Let(_, _, span) => span.clone(),
            Statement::Expr(_, span) => span.clone(),
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
            TokenKind::Ident(_) => {
                let (name, _) = self.parse_identifier()?;
                Ok(ident_to_value(name))
            }
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
            TokenKind::Ident(_) => {
                let (name, span) = self.parse_identifier()?;
                Ok(Term::Var(name, span))
            }
            TokenKind::LParen => {
                let lparen_span = token.span.clone();
                self.advance();
                let inner = self.parse_term()?;
                let rparen_span = match self.peek() {
                    Some(Token {
                        kind: TokenKind::RParen,
                        span,
                    }) => span.clone(),
                    Some(found) => {
                        return Err(ParseError::MissingToken {
                            expected: "')'",
                            pos: found.span.start,
                        });
                    }
                    None => {
                        return Err(ParseError::MissingToken {
                            expected: "')'",
                            pos: self.eof_pos(),
                        });
                    }
                };
                self.expect(TokenKind::RParen, "')'")?;
                Ok(inner.with_span(merge_span(&lparen_span, &rparen_span)))
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
            let span = merge_span(&left.span(), &right.span());
            left = Term::Application(Box::new(left), Box::new(right), span);
        }

        Ok(left)
    }

    fn parse_identifier(&mut self) -> Result<(String, Span), ParseError> {
        match self.peek() {
            Some(Token {
                kind: TokenKind::Ident(name),
                span,
            }) => {
                let name = name.clone();
                let span = span.clone();
                self.advance();
                Ok((name, span))
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
        let lambda_span = match self.peek() {
            Some(Token {
                kind: TokenKind::Lambda,
                span,
            }) => span.clone(),
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
        };
        self.expect_or_unexpected(TokenKind::Lambda, "lambda")?;

        let (identifier, _) = self.parse_identifier()?;

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

        let span = merge_span(&lambda_span, &body.span());
        Ok(Term::Lambda(identifier, ty, Box::new(body), span))
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
                let (name, value, let_span) = self.parse_let_head()?;
                let body = self.parse_scoped_let_tail()?;

                let span = merge_span(&let_span, &body.span());
                Ok(Term::Let {
                    name,
                    value: Box::new(value),
                    body: Box::new(body),
                    span,
                })
            }
            _ => self.parse_application(),
        }
    }

    fn parse_scoped_let_tail(&mut self) -> Result<Term, ParseError> {
        self.expect_or_unexpected(TokenKind::In, "in")?;
        self.parse_term()
    }

    fn parse_let_head(&mut self) -> Result<(String, Term, Span), ParseError> {
        let let_span = match self.peek() {
            Some(Token {
                kind: TokenKind::Let,
                span,
            }) => span.clone(),
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
        };
        self.expect_or_unexpected(TokenKind::Let, "let")?;

        let (name, _) = self.parse_identifier()?;

        self.expect_or_unexpected(TokenKind::Equals, "'='")?;

        let value = self.parse_term()?;

        Ok((name, value, let_span))
    }

    fn parse_statement(&mut self) -> Result<Statement, ParseError> {
        match self.peek() {
            Some(Token {
                kind: TokenKind::Let,
                ..
            }) => {
                let (name, value, let_span) = self.parse_let_head()?;

                let body = match self.peek() {
                    Some(Token {
                        kind: TokenKind::In,
                        ..
                    }) => self.parse_scoped_let_tail()?,
                    Some(Token {
                        kind: TokenKind::EOF,
                        ..
                    }) => {
                        let span = merge_span(&let_span, &value.span());
                        return Ok(Statement::Let(name.clone(), value, span));
                    }
                    Some(found) => {
                        return Err(ParseError::MissingToken {
                            expected: "in",
                            pos: found.span.start,
                        });
                    }
                    None => {
                        let span = merge_span(&let_span, &value.span());
                        return Ok(Statement::Let(name.clone(), value, span));
                    }
                };

                let span = merge_span(&let_span, &body.span());
                Ok(Statement::Expr(
                    Term::Let {
                        name,
                        value: Box::new(value),
                        body: Box::new(body),
                        span: span.clone(),
                    },
                    span,
                ))
            }
            _ => {
                let term = self.parse_term()?;
                let span = term.span();
                Ok(Statement::Expr(term, span))
            }
        }
    }
}

fn merge_span(start: &Span, end: &Span) -> Span {
    Span {
        start: start.start,
        end: end.end,
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
        lexer::{lex, Span, Token},
        parser::{parse, parse_term, ParseError, Statement, Term},
    };

    fn lex_and_parse(input: &str) -> Term {
        let tokens = lex(input).unwrap();
        parse_term(tokens).unwrap()
    }

    fn span() -> Span {
        Span { start: 0, end: 0 }
    }

    fn var(name: &str) -> Term {
        Term::Var(name.into(), span())
    }

    fn lam(name: &str, body: Term) -> Term {
        Term::Lambda(name.into(), None, Box::new(body), span())
    }

    fn app(left: Term, right: Term) -> Term {
        Term::Application(Box::new(left), Box::new(right), span())
    }

    fn let_term(name: &str, value: Term, body: Term) -> Term {
        Term::Let {
            name: name.into(),
            value: Box::new(value),
            body: Box::new(body),
            span: span(),
        }
    }

    fn assert_roundtrip(term: Term) {
        let rountrip = lex_and_parse(&term.to_string());
        assert_eq!(term, rountrip);
    }

    #[test]
    fn test_roundtrip() {
        assert_roundtrip(var("x"));
        assert_roundtrip(lam("x", app(var("x"), var("y"))));
        assert_roundtrip(app(lam("x", var("x")), var("y")));

        assert_roundtrip(app(app(var("f"), var("x")), var("y")));
        assert_roundtrip(app(var("f"), app(var("x"), var("y"))));
    }

    #[test]
    fn test_pretty_printer() {
        assert_eq!(app(lam("x", var("x")), var("y")).to_string(), "(\\x.x) y");

        assert_eq!(
            app(var("f"), app(var("x"), var("y"))).to_string(),
            "f (x y)"
        );
        assert_eq!(lam("x", app(var("x"), var("y"))).to_string(), "\\x.x y");
        assert_eq!(app(var("f"), lam("x", var("x"))).to_string(), "f (\\x.x)");
        assert_eq!(app(app(var("f"), var("x")), var("y")).to_string(), "f x y");
    }

    #[test]
    fn test_parser() {
        assert_eq!(lex_and_parse("x"), var("x"));
        assert_eq!(lex_and_parse("f x"), app(var("f"), var("x")));
        assert_eq!(
            lex_and_parse("f x y"),
            app(app(var("f"), var("x")), var("y"))
        );
        assert_eq!(lex_and_parse("(x)"), var("x"));
        assert_eq!(lex_and_parse("(\\x.x)"), lam("x", var("x")));

        assert_eq!(
            lex_and_parse("(\\x.x) y"),
            app(lam("x", var("x")), var("y"))
        );
        assert_eq!(lex_and_parse("\\x.x"), lam("x", var("x")));
        assert_eq!(
            lex_and_parse("f (\\x.x)"),
            app(var("f"), lam("x", var("x")))
        );
        assert_eq!(lex_and_parse("((x))"), var("x"));
        assert_eq!(lex_and_parse("\\x.x y"), lam("x", app(var("x"), var("y"))));
        assert_eq!(
            lex_and_parse("let id = \\x.x in id y"),
            let_term("id", lam("x", var("x")), app(var("id"), var("y")))
        );
    }

    #[test]
    fn test_parse_statement_global_let() {
        let tokens = lex("let x = y").unwrap();
        let statement = parse(tokens).unwrap();
        assert!(matches!(
            statement,
            Statement::Let(name, Term::Var(value, _), _) if name == "x" && value == "y"
        ));
    }

    #[test]
    fn test_application_span() {
        let term = lex_and_parse("(\\x.x) y");
        match term {
            Term::Application(left, right, span) => {
                assert_eq!(span.start, 0);
                assert_eq!(span.end, 8);
                assert_eq!(left.span().start, 0);
                assert_eq!(left.span().end, 6);
                assert_eq!(right.span().start, 7);
                assert_eq!(right.span().end, 8);
            }
            _ => panic!("expected application"),
        }
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
