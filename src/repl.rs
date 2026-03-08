use rustyline::DefaultEditor;

use crate::eval::evaluate;

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

                let result = evaluate(input);
                println!("{result}");
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
