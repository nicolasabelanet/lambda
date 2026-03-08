use rustyline::DefaultEditor;

use crate::{
    diagnostic::format_span_error,
    eval::{evaluate, EvalError},
};

pub fn repl() {
    let mut rl = DefaultEditor::new().unwrap();

    loop {
        let line = rl.readline("λ> ");

        match line {
            Ok(input) => {
                let input = input.trim();

                if input.is_empty() {
                    continue;
                }

                rl.add_history_entry(input).ok();

                match evaluate(input) {
                    Ok(result) => println!("{result}"),
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
    match err {
        EvalError::Lex(err) => {
            let message = err.message();
            let span = err.span();
            eprintln!("{}", format_span_error(source, &message, span));
        }
        EvalError::Parse(err) => {
            let message = err.message();
            let span = err.span();
            eprintln!("{}", format_span_error(source, &message, span));
        }
    }
}
