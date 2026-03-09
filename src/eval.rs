use std::collections::{HashMap, HashSet};

use crate::{
    lexer::{lex, LexError},
    parser::{parse, parse_term, ParseError, Statement, Term},
};

#[derive(Debug)]
pub enum EvalError {
    Lex(LexError),
    Parse(ParseError),
    StepLimit { limit: u32 },
}

impl std::fmt::Display for EvalError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            EvalError::Lex(err) => write!(f, "{}", err.message()),
            EvalError::Parse(err) => write!(f, "{}", err.message()),
            EvalError::StepLimit { limit } => {
                write!(f, "step limit reached ({limit})")
            }
        }
    }
}

impl From<LexError> for EvalError {
    fn from(err: LexError) -> Self {
        EvalError::Lex(err)
    }
}

impl From<ParseError> for EvalError {
    fn from(err: ParseError) -> Self {
        EvalError::Parse(err)
    }
}

pub struct Interpreter {
    env: HashMap<String, Term>,
    step_limit: u32,
}

impl Interpreter {
    pub fn new() -> Self {
        Interpreter {
            env: HashMap::new(),
            step_limit: 1_000,
        }
    }

    pub fn eval_statement(&mut self, input: &str) -> Result<Option<Term>, EvalError> {
        let ast = parse(lex(input)?)?;

        match ast {
            Statement::Let(name, term) => {
                let resolved = resolve(&term, &self.env);
                self.env
                    .insert(name, normalize_with_limit(&resolved, self.step_limit)?);
                Ok(None)
            }
            Statement::Expr(term) => {
                let resolved = resolve(&term, &self.env);
                let result = normalize_with_limit(&resolved, self.step_limit)?;
                self.env.insert("_".to_string(), result.clone());
                Ok(Some(result))
            }
        }
    }
}

impl Default for Interpreter {
    fn default() -> Self {
        Self::new()
    }
}

pub fn resolve(term: &Term, env: &HashMap<String, Term>) -> Term {
    let mut bound = HashSet::new();
    resolve_impl(term, env, &mut bound)
}

fn resolve_impl(term: &Term, env: &HashMap<String, Term>, bound: &mut HashSet<String>) -> Term {
    match term {
        Term::Var(name) => {
            if bound.contains(name) {
                term.clone()
            } else if let Some(global) = env.get(name) {
                capture_avoiding_clone(global, bound)
            } else {
                term.clone()
            }
        }
        Term::Lambda(param, body) => {
            bound.insert(param.clone());
            let body = resolve_impl(body, env, bound);
            bound.remove(param);
            Term::Lambda(param.clone(), Box::new(body))
        }
        Term::Application(left, right) => {
            let left = resolve_impl(left, env, bound);
            let right = resolve_impl(right, env, bound);
            Term::Application(Box::new(left), Box::new(right))
        }
        Term::Let { name, value, body } => {
            let value = resolve_impl(value, env, bound);
            bound.insert(name.clone());
            let body = resolve_impl(body, env, bound);
            bound.remove(name);
            Term::Let {
                name: name.clone(),
                value: Box::new(value),
                body: Box::new(body),
            }
        }
    }
}

pub fn evaluate(input: &str) -> Result<Term, EvalError> {
    let tokens = lex(input)?;
    let ast = parse_term(tokens)?;
    normalize(&ast)
}

fn normalize(term: &Term) -> Result<Term, EvalError> {
    normalize_with_limit(term, 1_000)
}

fn normalize_with_limit(term: &Term, limit: u32) -> Result<Term, EvalError> {
    let mut current = term.clone();

    let mut steps: u32 = 0;

    while let Some(reduced) = step(&current) {
        if steps >= limit {
            return Err(EvalError::StepLimit { limit });
        }
        steps += 1;
        current = reduced;
    }

    Ok(current)
}

fn is_value(term: &Term) -> bool {
    matches!(term, Term::Var(_) | Term::Lambda(_, _))
}

fn step(term: &Term) -> Option<Term> {
    match term {
        Term::Var(_) => None,
        Term::Lambda(_, _) => None,
        Term::Let { name, value, body } => {
            if is_value(value) {
                Some(substitute(body, name, value))
            } else {
                step(value).map(|new_value| Term::Let {
                    name: name.clone(),
                    value: Box::new(new_value),
                    body: body.clone(),
                })
            }
        }
        Term::Application(left, right) => match left.as_ref() {
            Term::Lambda(param, body) => {
                if is_value(right) {
                    Some(substitute(body, param, right))
                } else {
                    step(right)
                        .map(|new_right| Term::Application(left.clone(), Box::new(new_right)))
                }
            }
            _ => step(left).map(|new_left| Term::Application(Box::new(new_left), right.clone())),
        },
    }
}

fn rename(term: &Term, old: &str, new: &str) -> Term {
    match term {
        Term::Var(name) => {
            if name == old {
                Term::Var(new.to_string())
            } else {
                term.clone()
            }
        }
        Term::Application(left, right) => Term::Application(
            Box::new(rename(left, old, new)),
            Box::new(rename(right, old, new)),
        ),
        Term::Lambda(param, body) => {
            if param == old {
                term.clone()
            } else {
                Term::Lambda(param.clone(), Box::new(rename(body, old, new)))
            }
        }
        Term::Let { name, value, body } => {
            if name == old {
                Term::Let {
                    name: name.clone(),
                    value: Box::new(rename(value, old, new)),
                    body: body.clone(),
                }
            } else {
                Term::Let {
                    name: name.clone(),
                    value: Box::new(rename(value, old, new)),
                    body: Box::new(rename(body, old, new)),
                }
            }
        }
    }
}

fn capture_avoiding_clone(term: &Term, avoid: &HashSet<String>) -> Term {
    match term {
        Term::Var(_) => term.clone(),
        Term::Application(left, right) => Term::Application(
            Box::new(capture_avoiding_clone(left, avoid)),
            Box::new(capture_avoiding_clone(right, avoid)),
        ),
        Term::Lambda(param, body) => {
            if avoid.contains(param) {
                let mut used = avoid.clone();
                used.extend(free_vars(body));
                let fresh = create_fresh_name(param, &used);
                let renamed_body = rename(body, param, &fresh);
                let mut next_avoid = avoid.clone();
                next_avoid.insert(fresh.clone());
                Term::Lambda(
                    fresh,
                    Box::new(capture_avoiding_clone(&renamed_body, &next_avoid)),
                )
            } else {
                let mut next_avoid = avoid.clone();
                next_avoid.insert(param.clone());
                Term::Lambda(
                    param.clone(),
                    Box::new(capture_avoiding_clone(body, &next_avoid)),
                )
            }
        }
        Term::Let { name, value, body } => {
            let new_value = capture_avoiding_clone(value, avoid);
            if avoid.contains(name) {
                let mut used = avoid.clone();
                used.extend(free_vars(body));
                let fresh = create_fresh_name(name, &used);
                let renamed_body = rename(body, name, &fresh);
                let mut next_avoid = avoid.clone();
                next_avoid.insert(fresh.clone());
                Term::Let {
                    name: fresh,
                    value: Box::new(new_value),
                    body: Box::new(capture_avoiding_clone(&renamed_body, &next_avoid)),
                }
            } else {
                let mut next_avoid = avoid.clone();
                next_avoid.insert(name.clone());
                Term::Let {
                    name: name.clone(),
                    value: Box::new(new_value),
                    body: Box::new(capture_avoiding_clone(body, &next_avoid)),
                }
            }
        }
    }
}

fn update_lambda(lambda: &Term, new: &str) -> Term {
    match lambda {
        Term::Lambda(param, body) => {
            let renamed_body = rename(body, param, new);
            Term::Lambda(new.to_string(), Box::new(renamed_body))
        }
        _ => unreachable!("update_lambda called on non lambda"),
    }
}

fn create_fresh_name(base: &str, used: &HashSet<String>) -> String {
    if !used.contains(base) {
        return base.to_string();
    }

    let mut i: i32 = 1;

    loop {
        let cantidate = format!("{base}{i}");
        if !used.contains(&cantidate) {
            return cantidate;
        }
        i += 1;
    }
}

fn free_vars(term: &Term) -> HashSet<String> {
    match term {
        Term::Var(name) => HashSet::from([name.clone()]),
        Term::Application(left, right) => {
            let mut vars = free_vars(left);
            vars.extend(free_vars(right));
            vars
        }
        Term::Lambda(name, body) => {
            let mut body_vars = free_vars(body);
            body_vars.remove(name);
            body_vars
        }
        Term::Let { name, value, body } => {
            let mut vars = free_vars(value);
            let mut body_vars = free_vars(body);
            body_vars.remove(name);
            vars.extend(body_vars);
            vars
        }
    }
}

pub fn substitute(term: &Term, var: &str, replacement: &Term) -> Term {
    match term {
        Term::Var(name) => {
            if name == var {
                replacement.clone()
            } else {
                term.clone()
            }
        }
        Term::Application(left, right) => {
            let new_left = substitute(left, var, replacement);
            let new_right = substitute(right, var, replacement);
            Term::Application(Box::new(new_left), Box::new(new_right))
        }
        Term::Lambda(param, body) => {
            let free_replacement = free_vars(replacement);
            let free_body = free_vars(body);
            if param == var || !free_body.contains(var) {
                term.clone()
            } else if !free_replacement.contains(param) {
                Term::Lambda(param.clone(), Box::new(substitute(body, var, replacement)))
            } else {
                let mut used = free_replacement;
                used.extend(free_body);
                used.insert(param.clone());
                used.insert(var.to_string());
                let fresh_name = create_fresh_name(param, &used);
                match update_lambda(term, &fresh_name) {
                    Term::Lambda(new_param, new_body) => {
                        Term::Lambda(new_param, Box::new(substitute(&new_body, var, replacement)))
                    }
                    _ => unreachable!(),
                }
            }
        }
        Term::Let { name, value, body } => {
            let new_value = substitute(value, var, replacement);
            if name == var {
                Term::Let {
                    name: name.clone(),
                    value: Box::new(new_value),
                    body: body.clone(),
                }
            } else {
                let free_replacement = free_vars(replacement);
                if free_replacement.contains(name) {
                    let mut used = free_replacement;
                    used.extend(free_vars(body));
                    used.insert(name.clone());
                    used.insert(var.to_string());
                    let fresh_name = create_fresh_name(name, &used);
                    let renamed_body = rename(body, name, &fresh_name);
                    Term::Let {
                        name: fresh_name.clone(),
                        value: Box::new(new_value),
                        body: Box::new(substitute(&renamed_body, var, replacement)),
                    }
                } else {
                    Term::Let {
                        name: name.clone(),
                        value: Box::new(new_value),
                        body: Box::new(substitute(body, var, replacement)),
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use crate::{
        eval::{
            create_fresh_name, free_vars, normalize, rename, step, substitute, update_lambda,
            Interpreter,
        },
        lexer::lex,
        parser::{parse_term, Term},
    };

    fn ast(input: &str) -> Term {
        parse_term(lex(input).unwrap()).unwrap()
    }

    mod test_normalize {

        use super::*;
        #[test]
        fn test_normalize_respects_capture_avoidance() {
            assert_eq!(normalize(&ast("(\\x.\\y.x) y")).unwrap(), ast("\\y1.y"));

            assert_eq!(
                normalize(&ast("(\\x.\\y.x y) y")).unwrap(),
                ast("\\y1.y y1")
            );
        }

        #[test]
        fn test_normalize_simple() {
            assert_eq!(normalize(&ast("(\\x.x) y")).unwrap(), ast("y"));

            assert_eq!(normalize(&ast("(\\x.\\y.x) a")).unwrap(), ast("\\y.a"));

            assert_eq!(normalize(&ast("(\\x.x x) y")).unwrap(), ast("y y"));
        }

        #[test]
        fn test_normalize_let() {
            assert_eq!(normalize(&ast("let id = \\x.x in id y")).unwrap(), ast("y"));
            assert_eq!(
                normalize(&ast("let x = a in let x = b in x")).unwrap(),
                ast("b")
            );
        }

        #[test]
        fn test_normalize_multiple_steps() {
            assert_eq!(normalize(&ast("((\\f.f) (\\x.x)) y")).unwrap(), ast("y"));

            assert_eq!(normalize(&ast("(\\x.\\y.x) a b")).unwrap(), ast("a"));

            assert_eq!(normalize(&ast("(\\f.\\x.f x) g z")).unwrap(), ast("g z"));
        }

        #[test]
        fn test_normalize_under_call_by_name() {
            assert_eq!(normalize(&ast("(\\x.z) ((\\y.y) w)")).unwrap(), ast("z"));

            assert_eq!(normalize(&ast("(\\x.x) ((\\y.y) z)")).unwrap(), ast("z"));
        }
    }

    mod test_global_let {
        use super::*;

        #[test]
        fn test_interpreter_global_let() {
            let mut interpreter = Interpreter::new();
            assert_eq!(interpreter.eval_statement("let id = \\x.x").unwrap(), None);
            assert_eq!(interpreter.eval_statement("id z").unwrap(), Some(ast("z")));
        }
    }

    mod test_step {
        use super::*;

        #[test]
        fn test_simple_step() {
            assert_eq!(step(&ast("x")), None);
            assert_eq!(step(&ast("\\x.x")), None);
            assert_eq!(step(&ast("(\\x.x) y")), Some(ast("y")));
            assert_eq!(step(&ast("((\\f.f) (\\x.x)) y")), Some(ast("(\\x.x) y")));
            assert_eq!(step(&ast("f ((\\x.x) y)")), None);
        }

        #[test]
        fn test_step_stuck_terms() {
            assert_eq!(step(&ast("x")), None);
            assert_eq!(step(&ast("\\x.x")), None);
            assert_eq!(step(&ast("f x")), None);
        }

        #[test]
        fn test_step_simple_beta() {
            assert_eq!(step(&ast("(\\x.x) y")), Some(ast("y")));

            assert_eq!(step(&ast("(\\x.\\y.x) a")), Some(ast("\\y.a")));

            assert_eq!(step(&ast("(\\x.x x) y")), Some(ast("y y")));
        }

        #[test]
        fn test_step_reduces_left_side_of_application() {
            assert_eq!(step(&ast("((\\f.f) (\\x.x)) y")), Some(ast("(\\x.x) y")));

            assert_eq!(step(&ast("(((\\f.f) g) z)")), Some(ast("(g z)")));
        }

        #[test]
        fn test_step_call_by_name_does_not_reduce_argument() {
            assert_eq!(step(&ast("f ((\\x.x) y)")), None);

            assert_eq!(step(&ast("(\\x.z) ((\\y.y) w)")), Some(ast("(\\x.z) w")));
        }
    }

    mod test_update_lambda {
        use super::*;

        #[test]
        fn test_simple() {
            assert_eq!(
                update_lambda(
                    &Term::Lambda(
                        "y".into(),
                        Box::new(Term::Application(
                            Box::new(Term::Var("x".into())),
                            Box::new(Term::Var("y".into()))
                        ))
                    ),
                    "y1",
                ),
                Term::Lambda(
                    "y1".into(),
                    Box::new(Term::Application(
                        Box::new(Term::Var("x".into())),
                        Box::new(Term::Var("y1".into()))
                    ))
                )
            );
            assert_eq!(
                update_lambda(
                    &Term::Lambda(
                        "y".into(),
                        Box::new(Term::Lambda("y".into(), Box::new(Term::Var("y".into()))))
                    ),
                    "y1",
                ),
                Term::Lambda(
                    "y1".into(),
                    Box::new(Term::Lambda("y".into(), Box::new(Term::Var("y".into()))))
                ),
            );
        }
    }

    mod test_rename {
        use super::*;

        #[test]
        fn test_simple() {
            assert_eq!(
                rename(
                    &Term::Application(
                        Box::new(Term::Var("x".into())),
                        Box::new(Term::Var("y".into()))
                    ),
                    "y",
                    "y1"
                ),
                Term::Application(
                    Box::new(Term::Var("x".into())),
                    Box::new(Term::Var("y1".into()))
                )
            );
            assert_eq!(
                rename(
                    &Term::Lambda("y".into(), Box::new(Term::Var("y".into()))),
                    "y",
                    "z"
                ),
                Term::Lambda("y".into(), Box::new(Term::Var("y".into())))
            );
            assert_eq!(
                rename(
                    &Term::Lambda(
                        "z".into(),
                        Box::new(Term::Application(
                            Box::new(Term::Var("y".into())),
                            Box::new(Term::Var("z".into()))
                        ))
                    ),
                    "y",
                    "y1"
                ),
                Term::Lambda(
                    "z".into(),
                    Box::new(Term::Application(
                        Box::new(Term::Var("y1".into())),
                        Box::new(Term::Var("z".into()))
                    ))
                )
            );
        }
    }

    mod test_fresh_name {
        use super::*;

        #[test]
        fn test_simple() {
            assert_eq!(
                create_fresh_name(
                    "y",
                    &HashSet::from_iter(["y", "y1", "y2"].map(|s| s.to_string())),
                ),
                "y3".to_string()
            );

            assert_eq!(
                create_fresh_name(
                    "y",
                    &HashSet::from_iter(["y1", "y2"].map(|s| s.to_string())),
                ),
                "y".to_string()
            );
        }
    }

    mod test_substitute {
        use super::*;

        #[test]
        fn test_simple() {
            assert_eq!(substitute(&ast("x"), "x", &ast("y")), ast("y"));
            assert_eq!(substitute(&ast("z"), "x", &ast("y")), ast("z"));

            assert_eq!(substitute(&ast("x z"), "x", &ast("y")), ast("y z"));

            assert_eq!(substitute(&ast("x x"), "x", &ast("y")), ast("y y"));

            assert_eq!(substitute(&ast("x y"), "x", &ast("f z")), ast("(f z) y"));

            assert_eq!(
                substitute(&ast("f x"), "x", &ast("\\z.z")),
                ast("f (\\z.z)")
            );
        }

        #[test]
        fn test_substitute_shadowing() {
            assert_eq!(substitute(&ast("\\x.x"), "x", &ast("y")), ast("\\x.x"));

            assert_eq!(substitute(&ast("\\x.x z"), "x", &ast("y")), ast("\\x.x z"));

            assert_eq!(substitute(&ast("\\z.x"), "x", &ast("y")), ast("\\z.y"));

            assert_eq!(substitute(&ast("\\z.x z"), "x", &ast("y")), ast("\\z.y z"));

            assert_eq!(
                substitute(&ast("\\z.(\\x.x) x"), "x", &ast("y")),
                ast("\\z.(\\x.x) y")
            );
        }

        #[test]
        fn test_substitute_capture_avoidance_simple() {
            assert_eq!(substitute(&ast("\\y.x"), "x", &ast("y1")), ast("\\y.y1"));

            assert_eq!(substitute(&ast("\\y.x"), "x", &ast("y")), ast("\\y1.y"));

            assert_eq!(
                substitute(&ast("\\y.x y"), "x", &ast("y")),
                ast("\\y1.y y1")
            );

            assert_eq!(
                substitute(&ast("\\y.y x"), "x", &ast("y")),
                ast("\\y1.y1 y")
            );

            assert_eq!(
                substitute(&ast("\\y.f x y"), "x", &ast("y")),
                ast("\\y1.f y y1")
            );
        }

        #[test]
        fn test_substitute_capture_avoidance_with_compound_replacement() {
            assert_eq!(substitute(&ast("\\y.x"), "x", &ast("y z")), ast("\\y1.y z"));

            assert_eq!(
                substitute(&ast("\\y.x y"), "x", &ast("y z")),
                ast("\\y1.(y z) y1")
            );

            assert_eq!(
                substitute(&ast("\\y.f (x y)"), "x", &ast("y z")),
                ast("\\y1.f ((y z) y1)")
            );

            assert_eq!(
                substitute(&ast("\\y.x"), "x", &ast("\\z.y z")),
                ast("\\y1.\\z.y z")
            );

            assert_eq!(
                substitute(&ast("\\y.x y"), "x", &ast("\\z.y z")),
                ast("\\y1.(\\z.y z) y1")
            );
        }

        #[test]
        fn test_substitute_nested_lambdas() {
            assert_eq!(
                substitute(&ast("\\a.\\b.x a b"), "x", &ast("y")),
                ast("\\a.\\b.y a b")
            );

            assert_eq!(
                substitute(&ast("\\a.\\b.x a b"), "x", &ast("a")),
                ast("\\a1.\\b.a a1 b")
            );

            assert_eq!(
                substitute(&ast("\\y.\\z.x y z"), "x", &ast("y")),
                ast("\\y1.\\z.y y1 z")
            );

            assert_eq!(
                substitute(&ast("\\z.\\y.x y"), "x", &ast("y")),
                ast("\\z.\\y1.y y1")
            );

            assert_eq!(
                substitute(&ast("\\z.\\y.x z y"), "x", &ast("y")),
                ast("\\z.\\y1.y z y1")
            );
        }

        #[test]
        fn test_substitute_nested_shadowing() {
            assert_eq!(
                substitute(&ast("\\y.\\y.y"), "x", &ast("z")),
                ast("\\y.\\y.y")
            );

            assert_eq!(
                substitute(&ast("\\y.\\x.x"), "x", &ast("z")),
                ast("\\y.\\x.x")
            );

            assert_eq!(
                substitute(&ast("\\y.\\x.y x"), "x", &ast("z")),
                ast("\\y.\\x.y x")
            );

            assert_eq!(
                substitute(&ast("\\z.(\\x.x) x"), "x", &ast("y")),
                ast("\\z.(\\x.x) y")
            );

            assert_eq!(
                substitute(&ast("\\z.(\\x.x z) x"), "x", &ast("y")),
                ast("\\z.(\\x.x z) y")
            );
        }

        #[test]
        fn test_substitute_more_complex_applications() {
            assert_eq!(substitute(&ast("(x y) z"), "x", &ast("f")), ast("(f y) z"));

            assert_eq!(substitute(&ast("f (x y)"), "x", &ast("z")), ast("f (z y)"));

            assert_eq!(
                substitute(&ast("(x x) (x x)"), "x", &ast("y")),
                ast("(y y) (y y)")
            );

            assert_eq!(
                substitute(&ast("(\\z.z) x"), "x", &ast("y")),
                ast("(\\z.z) y")
            );

            assert_eq!(
                substitute(&ast("x (\\z.x z)"), "x", &ast("y")),
                ast("y (\\z.y z)")
            );
        }

        #[test]
        fn test_substitute_replacement_with_free_vars() {
            assert_eq!(substitute(&ast("\\a.x"), "x", &ast("y a")), ast("\\a1.y a"));

            assert_eq!(
                substitute(&ast("\\a.x a"), "x", &ast("y a")),
                ast("\\a1.(y a) a1")
            );

            assert_eq!(
                substitute(&ast("\\b.\\a.x b a"), "x", &ast("a")),
                ast("\\b.\\a1.a b a1")
            );

            assert_eq!(
                substitute(&ast("\\b.\\a.x a b"), "x", &ast("a")),
                ast("\\b.\\a1.a a1 b")
            );
        }

        #[test]
        fn test_substitute_identity_like_cases() {
            assert_eq!(
                substitute(&ast("\\x.\\y.x"), "x", &ast("z")),
                ast("\\x.\\y.x")
            );

            assert_eq!(
                substitute(&ast("\\z.\\y.z"), "x", &ast("y")),
                ast("\\z.\\y.z")
            );

            assert_eq!(
                substitute(&ast("\\z.\\y.x"), "x", &ast("y")),
                ast("\\z.\\y1.y")
            );

            assert_eq!(
                substitute(&ast("\\z.\\y.x z"), "x", &ast("y")),
                ast("\\z.\\y1.y z")
            );
        }

        #[test]
        fn test_naive_substitution() {
            assert_eq!(
                substitute(&Term::Var("x".into()), "x", &Term::Var("y".into())),
                Term::Var("y".into())
            );
            assert_eq!(
                substitute(&Term::Var("z".into()), "x", &Term::Var("y".into())),
                Term::Var("z".into())
            );
            assert_eq!(
                substitute(
                    &Term::Application(
                        Box::new(Term::Var("x".into())),
                        Box::new(Term::Var("z".into())),
                    ),
                    "x",
                    &Term::Var("y".into())
                ),
                Term::Application(
                    Box::new(Term::Var("y".into())),
                    Box::new(Term::Var("z".into())),
                )
            );
            assert_eq!(
                substitute(
                    &Term::Lambda("z".into(), Box::new(Term::Var("x".into())),),
                    "x",
                    &Term::Var("y".into())
                ),
                Term::Lambda("z".into(), Box::new(Term::Var("y".into())),),
            );

            assert_eq!(
                substitute(
                    &Term::Lambda("x".into(), Box::new(Term::Var("x".into())),),
                    "x",
                    &Term::Var("y".into())
                ),
                Term::Lambda("x".into(), Box::new(Term::Var("x".into())),),
            );
        }
    }

    mod test_free_vars {
        use super::*;

        #[test]
        fn test_free_vars() {
            assert_eq!(free_vars(&ast("x")), HashSet::from(["x".into()]));
            assert_eq!(free_vars(&ast("\\x.x")), HashSet::new());
            assert_eq!(free_vars(&ast("\\x.y")), HashSet::from(["y".into()]));
            assert_eq!(
                free_vars(&ast("f x")),
                HashSet::from(["f".into(), "x".into()])
            );
            assert_eq!(free_vars(&ast("\\x.x y")), HashSet::from(["y".into()]));
            assert_eq!(free_vars(&ast("(\\x.x) y")), HashSet::from(["y".into()]));
            assert_eq!(free_vars(&ast("\\x.\\y.x z")), HashSet::from(["z".into()]))
        }
    }
}
