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
    pub pos: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LexError {
    InvalidChar { ch: char, pos: usize },
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

    fn lex_ident(&mut self) -> Token {
        let mut ident = String::new();
        while let Some(c) = self.peek()
            && c.is_ascii_alphanumeric()
        {
            ident.push(c);
            self.advance();
        }

        Token::Ident(ident)
    }

    fn next_token(&mut self) -> Result<Token, LexError> {
        self.skip_whitespace();

        match self.peek() {
            Some(c) if c.is_ascii_alphanumeric() => Ok(self.lex_ident()),
            Some('\\') | Some('λ') => {
                self.advance();
                Ok(Token::Lambda)
            }
            Some('.') => {
                self.advance();
                Ok(Token::Dot)
            }
            Some('(') => {
                self.advance();
                Ok(Token::LParen)
            }
            Some(')') => {
                self.advance();
                Ok(Token::RParen)
            }
            Some(c) => Err(LexError::InvalidChar {
                ch: c,
                pos: self.pos,
            }),
            None => Ok(Token::EOF),
        }
    }

    fn peek(&mut self) -> Option<char> {
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

pub fn lex(input: &str) -> Vec<Token> {
    let mut tokens = Vec::new();
    let mut lexer = Lexer::new(input);

    loop {
        match lexer.next_token() {
            Ok(token) => {
                let mut end = false;

                if matches!(&token, Token::EOF) {
                    end = true;
                }
                tokens.push(token);

                if end {
                    break;
                }
            }
            Err(err) => {
                eprintln!("{err:?}");
                break;
            }
        }
    }

    tokens
}

#[cfg(test)]
mod tests {
    use crate::lexer::{Lexer, Token, lex};

    #[test]
    fn test_lone_ident() {
        assert_eq!(lex("x"), vec![Token::Ident("x".into()), Token::EOF]);
        assert_eq!(
            lex("xyz123"),
            vec![Token::Ident("xyz123".into()), Token::EOF]
        );
        assert_eq!(
            lex("123xyz"),
            vec![Token::Ident("123xyz".into()), Token::EOF]
        );
        assert_eq!(lex("123"), vec![Token::Ident("123".into()), Token::EOF]);
        assert_eq!(lex("xyz"), vec![Token::Ident("xyz".into()), Token::EOF]);
    }

    #[test]
    fn test_lone_lambda() {
        assert_eq!(lex("\\"), vec![Token::Lambda, Token::EOF]);
        assert_eq!(lex("λ"), vec![Token::Lambda, Token::EOF]);
    }

    #[test]
    fn test_lone_dot() {
        assert_eq!(lex("."), vec![Token::Dot, Token::EOF]);
    }

    #[test]
    fn test_lone_lparen() {
        assert_eq!(lex("("), vec![Token::LParen, Token::EOF]);
    }

    #[test]
    fn test_lone_rparen() {
        assert_eq!(lex(")"), vec![Token::RParen, Token::EOF]);
    }

    #[test]
    fn test_ident_boundary() {
        assert_eq!(
            lex("(abc)"),
            vec![
                Token::LParen,
                Token::Ident("abc".into()),
                Token::RParen,
                Token::EOF
            ]
        );
        assert_eq!(
            lex(".a.b.c."),
            vec![
                Token::Dot,
                Token::Ident("a".into()),
                Token::Dot,
                Token::Ident("b".into()),
                Token::Dot,
                Token::Ident("c".into()),
                Token::Dot,
                Token::EOF
            ]
        );

        assert_eq!(
            lex("a b c"),
            vec![
                Token::Ident("a".into()),
                Token::Ident("b".into()),
                Token::Ident("c".into()),
                Token::EOF
            ]
        );
    }

    #[test]
    fn test_whitespace_removal() {
        assert_eq!(
            lex("\n\t  \\x. x\r\n"),
            vec![
                Token::Lambda,
                Token::Ident("x".into()),
                Token::Dot,
                Token::Ident("x".into()),
                Token::EOF
            ]
        );

        assert_eq!(
            lex("              x    (         .       )          y"),
            vec![
                Token::Ident("x".into()),
                Token::LParen,
                Token::Dot,
                Token::RParen,
                Token::Ident("y".into()),
                Token::EOF
            ]
        );
    }

    #[test]
    fn test_empty() {
        assert_eq!(lex(""), vec![Token::EOF]);
    }

    #[test]
    fn test_whitespace_only() {
        assert_eq!(lex("          \t\t\t\t \r \n         \t"), vec![Token::EOF]);
    }

    #[test]
    fn test_expressions() {
        assert_eq!(
            lex("(\\x.x) y"),
            vec![
                Token::LParen,
                Token::Lambda,
                Token::Ident("x".into()),
                Token::Dot,
                Token::Ident("x".into()),
                Token::RParen,
                Token::Ident("y".into()),
                Token::EOF,
            ]
        );
    }

    #[test]
    fn test_eof_stability() {
        let mut lex = Lexer::new("x");
        assert_eq!(lex.next_token(), Ok(Token::Ident("x".into())));
        assert_eq!(lex.next_token(), Ok(Token::EOF));
        assert_eq!(lex.next_token(), Ok(Token::EOF));
        assert_eq!(lex.next_token(), Ok(Token::EOF));
    }
}
