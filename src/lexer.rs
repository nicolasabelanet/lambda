#[derive(Debug, PartialEq, Clone)]
pub enum Token {
    Lambda,
    Dot,
    LParen,
    RParen,
    Ident(String),
    EOF,
}

#[derive(Debug, PartialEq, Clone)]
pub enum TokenKind {
    Lambda,
    Dot,
    LParen,
    RParen,
    Ident(String),
    EOF,
}

#[derive(Debug, PartialEq, Clone)]
pub struct TokenSpan {
    pub kind: TokenKind,
    pub start: usize,
    pub end: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LexError {
    InvalidChar { ch: char, span: Span },
}

impl LexError {
    pub fn span(&self) -> Span {
        match self {
            LexError::InvalidChar { span, .. } => span.clone(),
        }
    }

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

struct Lexer {
    pos: usize,
    input: Vec<char>,
}

impl Lexer {
    fn new(input: &str) -> Lexer {
        Lexer {
            pos: 0,
            input: input.chars().collect(),
        }
    }

    fn lex_ident(&mut self) -> TokenSpan {
        let start = self.pos;
        let mut ident = String::new();
        while let Some(c) = self.peek()
            && c.is_ascii_alphanumeric()
        {
            ident.push(c);
            self.advance();
        }
        let end = self.pos;

        TokenSpan {
            kind: TokenKind::Ident(ident),
            start,
            end,
        }
    }

    fn next_token(&mut self) -> Result<TokenSpan, LexError> {
        self.skip_whitespace();

        let start = self.pos;

        match self.peek() {
            Some(c) if c.is_ascii_alphanumeric() => Ok(self.lex_ident()),
            Some('\\') | Some('λ') => {
                self.advance();
                Ok(TokenSpan {
                    kind: TokenKind::Lambda,
                    start,
                    end: self.pos,
                })
            }
            Some('.') => {
                self.advance();
                Ok(TokenSpan {
                    kind: TokenKind::Dot,
                    start,
                    end: self.pos,
                })
            }
            Some('(') => {
                self.advance();
                Ok(TokenSpan {
                    kind: TokenKind::LParen,
                    start,
                    end: self.pos,
                })
            }
            Some(')') => {
                self.advance();
                Ok(TokenSpan {
                    kind: TokenKind::RParen,
                    start,
                    end: self.pos,
                })
            }
            Some(c) => Err(LexError::InvalidChar {
                ch: c,
                span: Span {
                    start,
                    end: self.pos + 1,
                },
            }),
            None => Ok(TokenSpan {
                kind: TokenKind::EOF,
                start: self.pos,
                end: self.pos,
            }),
        }
    }

    fn peek(&self) -> Option<char> {
        if self.pos >= self.input.len() {
            return None;
        }

        self.input.get(self.pos).copied()
    }

    fn advance(&mut self) {
        if self.pos < self.input.len() {
            self.pos += 1;
        }
    }

    fn skip_whitespace(&mut self) {
        while let Some(c) = self.peek()
            && c.is_whitespace()
        {
            self.advance();
        }
    }
}

pub fn lex(input: &str) -> Result<Vec<TokenSpan>, LexError> {
    let mut tokens = Vec::new();
    let mut lexer = Lexer::new(input);

    loop {
        let result = lexer.next_token();
        match result {
            Ok(token) => {
                let mut end = false;

                if matches!(
                    &token,
                    TokenSpan {
                        kind: TokenKind::EOF,
                        ..
                    }
                ) {
                    end = true;
                }
                tokens.push(token);

                if end {
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
    use crate::lexer::{Lexer, TokenKind, TokenSpan, lex};

    fn kinds(tokens: Vec<TokenSpan>) -> Vec<TokenKind> {
        tokens.into_iter().map(|token| token.kind).collect()
    }

    #[test]
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
    fn test_lone_dot() {
        assert_eq!(
            kinds(lex(".").unwrap()),
            vec![TokenKind::Dot, TokenKind::EOF]
        );
    }

    #[test]
    fn test_lone_lparen() {
        assert_eq!(
            kinds(lex("(").unwrap()),
            vec![TokenKind::LParen, TokenKind::EOF]
        );
    }

    #[test]
    fn test_lone_rparen() {
        assert_eq!(
            kinds(lex(")").unwrap()),
            vec![TokenKind::RParen, TokenKind::EOF]
        );
    }

    #[test]
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
    fn test_empty() {
        assert_eq!(kinds(lex("").unwrap()), vec![TokenKind::EOF]);
    }

    #[test]
    fn test_whitespace_only() {
        assert_eq!(
            kinds(lex("          \t\t\t\t \r \n         \t").unwrap()),
            vec![TokenKind::EOF]
        );
    }

    #[test]
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
    fn test_eof_stability() {
        let mut lex = Lexer::new("x");
        assert_eq!(lex.next_token().unwrap().kind, TokenKind::Ident("x".into()));
        assert_eq!(lex.next_token().unwrap().kind, TokenKind::EOF);
        assert_eq!(lex.next_token().unwrap().kind, TokenKind::EOF);
        assert_eq!(lex.next_token().unwrap().kind, TokenKind::EOF);
    }

    #[test]
    fn test_invalid_char() {
        assert!(lex("@").is_err());
    }
}
