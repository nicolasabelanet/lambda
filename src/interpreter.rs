use std::collections::HashMap;

use crate::{
    eval::{normalize_with_limit, resolve, EvalError, EvalMode},
    lexer::lex,
    parser::{parse, Statement, Term},
    typing::{
        infer_statement, seed_free_vars_statement, Type, TypeEnv, TypeScheme, TypeVarGenerator,
    },
    util::term,
};

/// Returns the standard library term environment.
fn stdlib() -> HashMap<String, Term> {
    HashMap::from_iter([
        ("true".to_string(), term(r"\t.\f.t")),
        ("false".to_string(), term(r"\t.\f.f")),
        ("if".to_string(), term(r"\b.\t.\f. b t f")),
        ("and".to_string(), term(r"\p.\q. p q p")),
        ("or".to_string(), term(r"\p.\q. p p q")),
        ("not".to_string(), term(r"\p. p false true")),
    ])
}

/// Returns the standard library type environment.
fn stdlib_types() -> TypeEnv {
    TypeEnv::from_iter([
        (
            "true".into(),
            TypeScheme {
                vars: vec![],
                ty: Type::Bool,
            },
        ),
        (
            "false".into(),
            TypeScheme {
                vars: vec![],
                ty: Type::Bool,
            },
        ),
        (
            "not".into(),
            TypeScheme {
                vars: vec![],
                ty: Type::Arrow(Box::new(Type::Bool), Box::new(Type::Bool)),
            },
        ),
        (
            "and".into(),
            TypeScheme {
                vars: vec![],
                ty: Type::Arrow(
                    Box::new(Type::Bool),
                    Box::new(Type::Arrow(Box::new(Type::Bool), Box::new(Type::Bool))),
                ),
            },
        ),
        (
            "or".into(),
            TypeScheme {
                vars: vec![],
                ty: Type::Arrow(
                    Box::new(Type::Bool),
                    Box::new(Type::Arrow(Box::new(Type::Bool), Box::new(Type::Bool))),
                ),
            },
        ),
        (
            "if".into(),
            TypeScheme {
                vars: vec![],
                ty: Type::Arrow(
                    Box::new(Type::Bool),
                    Box::new(Type::Arrow(
                        Box::new(Type::Bool),
                        Box::new(Type::Arrow(Box::new(Type::Bool), Box::new(Type::Bool))),
                    )),
                ),
            },
        ),
    ])
}

pub struct Interpreter {
    env: HashMap<String, Term>,
    type_env: TypeEnv,
    step_limit: u32,
    eval_mode: EvalMode,
}

impl Interpreter {
    /// Creates a new interpreter with the given evaluation mode.
    pub fn new(eval_mode: EvalMode) -> Self {
        Interpreter {
            env: stdlib(),
            type_env: stdlib_types(),
            step_limit: 1_000,
            eval_mode,
        }
    }

    /// Parses, type-checks, and evaluates a statement.
    pub fn eval_statement(&mut self, input: &str) -> Result<Option<(Term, Type)>, EvalError> {
        let ast = parse(lex(input)?)?;

        let mut generator = TypeVarGenerator::new();
        seed_free_vars_statement(&ast, &mut self.type_env, &mut generator);
        let inferred_ty = infer_statement(&ast, &mut self.type_env, &mut generator)?;

        match ast {
            Statement::Let(name, term, _) => {
                let resolved = resolve(&term, &self.env);
                self.env.insert(
                    name,
                    normalize_with_limit(&resolved, self.step_limit, self.eval_mode)?,
                );
                Ok(None)
            }
            Statement::Expr(term, _) => {
                let resolved = resolve(&term, &self.env);
                let result = normalize_with_limit(&resolved, self.step_limit, self.eval_mode)?;
                self.env.insert("_".to_string(), result.clone());
                let inferred_ty = inferred_ty.expect("expression should infer a type");
                Ok(Some((result, inferred_ty)))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    mod test_global_let {
        use crate::{eval::EvalMode, interpreter::Interpreter, util::term};

        #[test]
        /// Ensures global let bindings are evaluated and stored.
        fn test_interpreter_global_let() {
            let mut interpreter = Interpreter::new(EvalMode::CallByValue);
            assert_eq!(interpreter.eval_statement("let id = \\x.x").unwrap(), None);
            let (result, _ty) = interpreter.eval_statement("id z").unwrap().unwrap();
            assert_eq!(result, term("z"));
        }
    }
}
