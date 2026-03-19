#[derive(Debug, PartialEq, Clone)]
pub enum TokenKind {
    Lambda,
    Dot,
    Let,
    Equals,
    In,
    LParen,
    RParen,
    Arrow,
    Colon,
    Ident(String),
    EOF,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LexError {
    InvalidChar { ch: char, span: Span },
}

impl LexError {
    /// Returns the span associated with this lex error.
    pub fn span(&self) -> Span {
        match self {
            LexError::InvalidChar { span, .. } => span.clone(),
        }
    }

    /// Returns a human-friendly error message.
    pub fn message(&self) -> String {
        match self {
            LexError::InvalidChar { ch, .. } => format!("invalid character '{ch}'"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Span {
    pub start: usize,
    pub end: usize,
}

#[derive(Debug, PartialEq, Clone)]
pub struct Token {
    pub kind: TokenKind,
    pub span: Span,
}

struct Lexer {
    pos: usize,
    input: Vec<char>,
}

/// Returns true if a character is valid in identifiers.
fn is_ident_char(c: char) -> bool {
    c == '_' || c.is_ascii_alphanumeric()
}

impl Lexer {
    /// Creates a new lexer for the input string.
    fn new(input: &str) -> Lexer {
        Lexer {
            pos: 0,
            input: input.chars().collect(),
        }
    }

    /// Lexes an identifier or keyword token.
    fn lex_ident(&mut self) -> Token {
        let start = self.pos;
        let mut ident = String::new();
        while let Some(c) = self.peek()
            && is_ident_char(c)
        {
            ident.push(c);
            self.advance();
        }
        let end = self.pos;

        let span = Span { start, end };

        match ident.as_str() {
            "in" => Token {
                kind: TokenKind::In,
                span,
            },
            "let" => Token {
                kind: TokenKind::Let,
                span,
            },
            _ => Token {
                kind: TokenKind::Ident(ident),
                span,
            },
        }
    }

    /// Builds a token with the current cursor span.
    fn simple_token(&self, kind: TokenKind, start: usize) -> Token {
        Token {
            kind,
            span: Span {
                start,
                end: self.pos,
            },
        }
    }

    /// Lexes the next token from the input stream.
    fn next_token(&mut self) -> Result<Token, LexError> {
        self.skip_whitespace();

        let start = self.pos;

        match self.peek() {
            Some(c) if is_ident_char(c) => Ok(self.lex_ident()),
            Some('\\') | Some('λ') => {
                self.advance();
                Ok(self.simple_token(TokenKind::Lambda, start))
            }
            Some(':') => {
                self.advance();
                Ok(self.simple_token(TokenKind::Colon, start))
            }
            Some('-') => {
                let next = self.peek_n(1);
                if next == Some('>') {
                    self.advance();
                    self.advance();
                    Ok(self.simple_token(TokenKind::Arrow, start))
                } else {
                    Err(LexError::InvalidChar {
                        ch: '-',
                        span: Span {
                            start,
                            end: self.pos + 1,
                        },
                    })
                }
            }
            Some('.') => {
                self.advance();
                Ok(self.simple_token(TokenKind::Dot, start))
            }
            Some('=') => {
                self.advance();
                Ok(self.simple_token(TokenKind::Equals, start))
            }
            Some('(') => {
                self.advance();
                Ok(self.simple_token(TokenKind::LParen, start))
            }
            Some(')') => {
                self.advance();
                Ok(self.simple_token(TokenKind::RParen, start))
            }
            Some(c) => Err(LexError::InvalidChar {
                ch: c,
                span: Span {
                    start,
                    end: self.pos + 1,
                },
            }),
            None => Ok(Token {
                kind: TokenKind::EOF,
                span: Span {
                    start: self.pos,
                    end: self.pos,
                },
            }),
        }
    }

    /// Peeks ahead by offset characters without consuming.
    fn peek_n(&self, offset: usize) -> Option<char> {
        self.input.get(self.pos + offset).copied()
    }

    /// Peeks at the current character without consuming it.
    fn peek(&self) -> Option<char> {
        self.input.get(self.pos).copied()
    }

    /// Advances the cursor by one character.
    fn advance(&mut self) {
        if self.pos < self.input.len() {
            self.pos += 1;
        }
    }

    /// Advances past whitespace characters.
    fn skip_whitespace(&mut self) {
        while let Some(c) = self.peek()
            && c.is_whitespace()
        {
            self.advance();
        }
    }
}

/// Lexes an input string into a list of tokens.
pub fn lex(input: &str) -> Result<Vec<Token>, LexError> {
    let mut tokens = Vec::new();
    let mut lexer = Lexer::new(input);

    loop {
        let result = lexer.next_token();
        match result {
            Ok(token) => {
                let is_eof = matches!(&token.kind, TokenKind::EOF);
                tokens.push(token);
                if is_eof {
                    break;
                }
            }
            Err(err) => return Err(err),
        }
    }

    Ok(tokens)
}

#[cfg(test)]
mod tests {
    use crate::lexer::{Lexer, Token, TokenKind, lex};

    fn kinds(tokens: Vec<Token>) -> Vec<TokenKind> {
        tokens.into_iter().map(|token| token.kind).collect()
    }

    #[test]
    /// Ensures identifiers lex correctly.
    fn test_lone_ident() {
        assert_eq!(
            kinds(lex("x").unwrap()),
            vec![TokenKind::Ident("x".into()), TokenKind::EOF]
        );
        assert_eq!(
            kinds(lex("xyz123").unwrap()),
            vec![TokenKind::Ident("xyz123".into()), TokenKind::EOF]
        );
        assert_eq!(
            kinds(lex("123xyz").unwrap()),
            vec![TokenKind::Ident("123xyz".into()), TokenKind::EOF]
        );
        assert_eq!(
            kinds(lex("123").unwrap()),
            vec![TokenKind::Ident("123".into()), TokenKind::EOF]
        );
        assert_eq!(
            kinds(lex("xyz").unwrap()),
            vec![TokenKind::Ident("xyz".into()), TokenKind::EOF]
        );
    }

    #[test]
    /// Ensures lambda tokens lex correctly.
    fn test_lone_lambda() {
        assert_eq!(
            kinds(lex("\\").unwrap()),
            vec![TokenKind::Lambda, TokenKind::EOF]
        );
        assert_eq!(
            kinds(lex("λ").unwrap()),
            vec![TokenKind::Lambda, TokenKind::EOF]
        );
    }

    #[test]
    /// Ensures dot tokens lex correctly.
    fn test_lone_dot() {
        assert_eq!(
            kinds(lex(".").unwrap()),
            vec![TokenKind::Dot, TokenKind::EOF]
        );
    }

    #[test]
    /// Ensures left paren tokens lex correctly.
    fn test_lone_lparen() {
        assert_eq!(
            kinds(lex("(").unwrap()),
            vec![TokenKind::LParen, TokenKind::EOF]
        );
    }

    #[test]
    /// Ensures right paren tokens lex correctly.
    fn test_lone_rparen() {
        assert_eq!(
            kinds(lex(")").unwrap()),
            vec![TokenKind::RParen, TokenKind::EOF]
        );
    }

    #[test]
    /// Ensures keywords and equals tokens lex correctly.
    fn test_keywords_and_equals() {
        assert_eq!(
            kinds(lex("let").unwrap()),
            vec![TokenKind::Let, TokenKind::EOF]
        );
        assert_eq!(
            kinds(lex("in").unwrap()),
            vec![TokenKind::In, TokenKind::EOF]
        );
        assert_eq!(
            kinds(lex("=").unwrap()),
            vec![TokenKind::Equals, TokenKind::EOF]
        );
        assert_eq!(
            kinds(lex("letx").unwrap()),
            vec![TokenKind::Ident("letx".into()), TokenKind::EOF]
        );
        assert_eq!(
            kinds(lex("in1").unwrap()),
            vec![TokenKind::Ident("in1".into()), TokenKind::EOF]
        );
    }

    #[test]
    /// Ensures let expression tokens lex correctly.
    fn test_let_expression_tokens() {
        assert_eq!(
            kinds(lex("let x = \\y.y in x").unwrap()),
            vec![
                TokenKind::Let,
                TokenKind::Ident("x".into()),
                TokenKind::Equals,
                TokenKind::Lambda,
                TokenKind::Ident("y".into()),
                TokenKind::Dot,
                TokenKind::Ident("y".into()),
                TokenKind::In,
                TokenKind::Ident("x".into()),
                TokenKind::EOF
            ]
        );
    }

    #[test]
    /// Ensures identifier boundaries are handled correctly.
    fn test_ident_boundary() {
        assert_eq!(
            kinds(lex("(abc)").unwrap()),
            vec![
                TokenKind::LParen,
                TokenKind::Ident("abc".into()),
                TokenKind::RParen,
                TokenKind::EOF
            ]
        );
        assert_eq!(
            kinds(lex(".a.b.c.").unwrap()),
            vec![
                TokenKind::Dot,
                TokenKind::Ident("a".into()),
                TokenKind::Dot,
                TokenKind::Ident("b".into()),
                TokenKind::Dot,
                TokenKind::Ident("c".into()),
                TokenKind::Dot,
                TokenKind::EOF
            ]
        );

        assert_eq!(
            kinds(lex("a b c").unwrap()),
            vec![
                TokenKind::Ident("a".into()),
                TokenKind::Ident("b".into()),
                TokenKind::Ident("c".into()),
                TokenKind::EOF
            ]
        );
    }

    #[test]
    /// Ensures whitespace is skipped by the lexer.
    fn test_whitespace_removal() {
        assert_eq!(
            kinds(lex("\n\t  \\x. x\r\n").unwrap()),
            vec![
                TokenKind::Lambda,
                TokenKind::Ident("x".into()),
                TokenKind::Dot,
                TokenKind::Ident("x".into()),
                TokenKind::EOF
            ]
        );

        assert_eq!(
            kinds(lex("              x    (         .       )          y").unwrap()),
            vec![
                TokenKind::Ident("x".into()),
                TokenKind::LParen,
                TokenKind::Dot,
                TokenKind::RParen,
                TokenKind::Ident("y".into()),
                TokenKind::EOF
            ]
        );
    }

    #[test]
    /// Ensures empty input lexes to EOF.
    fn test_empty() {
        assert_eq!(kinds(lex("").unwrap()), vec![TokenKind::EOF]);
    }

    #[test]
    /// Ensures whitespace-only input lexes to EOF.
    fn test_whitespace_only() {
        assert_eq!(
            kinds(lex("          \t\t\t\t \r \n         \t").unwrap()),
            vec![TokenKind::EOF]
        );
    }

    #[test]
    /// Ensures general expressions lex correctly.
    fn test_expressions() {
        assert_eq!(
            kinds(lex("(\\x.x) y").unwrap()),
            vec![
                TokenKind::LParen,
                TokenKind::Lambda,
                TokenKind::Ident("x".into()),
                TokenKind::Dot,
                TokenKind::Ident("x".into()),
                TokenKind::RParen,
                TokenKind::Ident("y".into()),
                TokenKind::EOF,
            ]
        );
    }

    #[test]
    /// Ensures colon and arrow tokens lex correctly.
    fn test_colon_and_arrow_tokens() {
        assert_eq!(
            kinds(lex("x: A -> B").unwrap()),
            vec![
                TokenKind::Ident("x".into()),
                TokenKind::Colon,
                TokenKind::Ident("A".into()),
                TokenKind::Arrow,
                TokenKind::Ident("B".into()),
                TokenKind::EOF,
            ]
        );
    }

    #[test]
    /// Ensures EOF is stable across repeated lexing.
    fn test_eof_stability() {
        let mut lex = Lexer::new("x");
        assert_eq!(lex.next_token().unwrap().kind, TokenKind::Ident("x".into()));
        assert_eq!(lex.next_token().unwrap().kind, TokenKind::EOF);
        assert_eq!(lex.next_token().unwrap().kind, TokenKind::EOF);
        assert_eq!(lex.next_token().unwrap().kind, TokenKind::EOF);
    }

    #[test]
    /// Ensures invalid characters yield a lex error.
    fn test_invalid_char() {
        let err = lex("@").unwrap_err();
        assert_eq!(err.message(), "invalid character '@'");
        assert_eq!(err.span().start, 0);
        assert_eq!(err.span().end, 1);
    }
}
