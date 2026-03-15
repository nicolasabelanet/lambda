use std::collections::HashMap;

use crate::{
    eval::{EvalError, EvalMode, normalize_with_limit, resolve},
    lexer::lex,
    parser::{Statement, Term, parse, stmt, term},
};

fn stdlib() -> HashMap<String, Term> {
    HashMap::from_iter([
        ("true".to_string(), term("\\t.\\f.t")),
        ("false".to_string(), term("\\t.\\f.f")),
        ("if".to_string(), term("\\b.\\t.\\f. b t f")),
        ("and".to_string(), term("\\p.\\q. p q p")),
        ("or".to_string(), term("\\p.\\q. p p q")),
        ("not".to_string(), term("\\p. p false true")),
    ])
}

pub struct Interpreter {
    env: HashMap<String, Term>,
    step_limit: u32,
    eval_mode: EvalMode,
}

impl Interpreter {
    pub fn new(eval_mode: EvalMode) -> Self {
        Interpreter {
            env: stdlib(),
            step_limit: 1_000,
            eval_mode,
        }
    }

    pub fn eval_statement(&mut self, input: &str) -> Result<Option<Term>, EvalError> {
        let ast = parse(lex(input)?)?;
        match ast {
            Statement::Let(name, term) => {
                let resolved = resolve(&term, &self.env);
                self.env.insert(
                    name,
                    normalize_with_limit(&resolved, self.step_limit, self.eval_mode)?,
                );
                Ok(None)
            }
            Statement::Expr(term) => {
                let resolved = resolve(&term, &self.env);
                let result = normalize_with_limit(&resolved, self.step_limit, self.eval_mode)?;
                self.env.insert("_".to_string(), result.clone());
                Ok(Some(result))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    mod test_global_let {
        use crate::{eval::EvalMode, interpreter::Interpreter, parser::term};

        #[test]
        fn test_interpreter_global_let() {
            let mut interpreter = Interpreter::new(EvalMode::CallByValue);
            assert_eq!(interpreter.eval_statement("let id = \\x.x").unwrap(), None);
            assert_eq!(interpreter.eval_statement("id z").unwrap(), Some(term("z")));
        }
    }
}
