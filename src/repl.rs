use rustyline::DefaultEditor;

use crate::{
    diagnostic::format_span_error,
    eval::{EvalError, Interpreter},
};

pub fn repl() {
    let mut rl = DefaultEditor::new().unwrap();

    let mut interpreter = Interpreter::new();

    loop {
        let line = rl.readline("λ> ");

        match line {
            Ok(input) => {
                let input = input.trim();

                if input.is_empty() {
                    continue;
                }

                rl.add_history_entry(input).ok();

                match interpreter.eval_statement(input) {
                    Ok(Some(result)) => println!("{result}"),
                    Ok(None) => {}
                    Err(err) => print_error(input, err),
                }
            }
            Err(rustyline::error::ReadlineError::Interrupted) => {
                break;
            }
            Err(rustyline::error::ReadlineError::Eof) => {
                break;
            }
            Err(err) => {
                eprintln!("error: {err}");
                break;
            }
        }
    }
}

fn print_error(source: &str, err: EvalError) {
    eprintln!("{}", format_eval_error(source, err));
}

fn format_eval_error(source: &str, err: EvalError) -> String {
    match err {
        EvalError::Lex(err) => {
            let message = err.message();
            let span = err.span();
            format_span_error(source, &message, span)
        }
        EvalError::Parse(err) => {
            let message = err.message();
            let span = err.span();
            format_span_error(source, &message, span)
        }
        _ => format!("error: {err}"),
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        eval::EvalError,
        lexer::{LexError, Span, Token, TokenKind},
        parser::ParseError,
        repl::format_eval_error,
    };

    #[test]
    fn test_format_eval_error_lex() {
        let source = "@";
        let err = EvalError::Lex(LexError::InvalidChar {
            ch: '@',
            span: Span { start: 0, end: 1 },
        });
        let output = format_eval_error(source, err);
        let expected = "error: invalid character '@'\n --> line 1, col 1\n  |\n1 | @\n  | ^";
        assert_eq!(output, expected);
    }

    #[test]
    fn test_format_eval_error_parse_missing_dot() {
        let source = "\\x";
        let err = EvalError::Parse(ParseError::MissingToken {
            expected: "'.'",
            pos: 2,
        });
        let output = format_eval_error(source, err);
        let expected = "error: expected '.'\n --> line 1, col 3\n  |\n1 | \\x\n  |   ^";
        assert_eq!(output, expected);
    }

    #[test]
    fn test_format_eval_error_parse_unexpected_token() {
        let source = "x)";
        let err = EvalError::Parse(ParseError::UnexpectedToken {
            expected: "end of input",
            found: Token {
                kind: TokenKind::RParen,
                span: Span { start: 1, end: 2 },
            },
        });
        let output = format_eval_error(source, err);
        let expected = "error: expected end of input\n --> line 1, col 2\n  |\n1 | x)\n  |  ^";
        assert_eq!(output, expected);
    }
}
