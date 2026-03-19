use std::collections::HashMap;

use crate::{
    eval::{normalize_with_limit, resolve, EvalError, EvalMode},
    lexer::lex,
    parser::{parse, Statement, Term},
    typing::{
        infer_statement, seed_free_vars_statement, Type, TypeEnv, TypeScheme, TypeVar,
        TypeVarGenerator,
    },
    util::term,
};

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

fn church_bool_scheme() -> TypeScheme {
    // ∀a. a -> a -> a
    let a = 0u32;
    TypeScheme {
        vars: vec![a],
        ty: Type::Arrow(
            Box::new(Type::Meta(a)),
            Box::new(Type::Arrow(
                Box::new(Type::Meta(a)),
                Box::new(Type::Meta(a)),
            )),
        ),
    }
}
fn stdlib_types() -> TypeEnv {
    // Helper for (a -> a -> a)
    let bool_ty = |a: TypeVar| {
        Type::Arrow(
            Box::new(Type::Meta(a)),
            Box::new(Type::Arrow(
                Box::new(Type::Meta(a)),
                Box::new(Type::Meta(a)),
            )),
        )
    };
    // ∀a. (a -> a -> a) -> a -> a -> a
    let if_scheme = {
        let a = 0u32;
        TypeScheme {
            vars: vec![a],
            ty: Type::Arrow(
                Box::new(bool_ty(a)),
                Box::new(Type::Arrow(
                    Box::new(Type::Meta(a)),
                    Box::new(Type::Arrow(
                        Box::new(Type::Meta(a)),
                        Box::new(Type::Meta(a)),
                    )),
                )),
            ),
        }
    };
    // ∀a. (a -> a -> a) -> (a -> a -> a) -> (a -> a -> a)
    let bin_bool_scheme = {
        let a = 0u32;
        let bool_a = bool_ty(a);
        TypeScheme {
            vars: vec![a],
            ty: Type::Arrow(
                Box::new(bool_a.clone()),
                Box::new(Type::Arrow(Box::new(bool_a.clone()), Box::new(bool_a))),
            ),
        }
    };
    TypeEnv::from_iter([
        ("true".into(), church_bool_scheme()),
        ("false".into(), church_bool_scheme()),
        ("if".into(), if_scheme),
        ("and".into(), bin_bool_scheme.clone()),
        ("or".into(), bin_bool_scheme.clone()),
        ("not".into(), bin_bool_scheme), // (Bool -> Bool) with Church Bool
    ])
}

pub struct Interpreter {
    env: HashMap<String, Term>,
    type_env: TypeEnv,
    step_limit: u32,
    eval_mode: EvalMode,
}

impl Interpreter {
    pub fn new(eval_mode: EvalMode) -> Self {
        Interpreter {
            env: stdlib(),
            type_env: stdlib_types(),
            step_limit: 1_000,
            eval_mode,
        }
    }

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
        fn test_interpreter_global_let() {
            let mut interpreter = Interpreter::new(EvalMode::CallByValue);
            assert_eq!(interpreter.eval_statement("let id = \\x.x").unwrap(), None);
            let (result, _ty) = interpreter.eval_statement("id z").unwrap().unwrap();
            assert_eq!(result, term("z"));
        }
    }
}
