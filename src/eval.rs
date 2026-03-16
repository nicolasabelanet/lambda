use std::collections::{HashMap, HashSet};

use crate::{
    lexer::{LexError, lex},
    parser::{ParseError, Term, parse_term},
    typing::{
        Type, TypeEnv, TypeError, TypeVarGenerator, infer, seed_free_vars_statement,
        seed_free_vars_term,
    },
};

#[derive(Debug)]
pub enum EvalError {
    Lex(LexError),
    Type(TypeError),
    Parse(ParseError),
    StepLimit { limit: u32 },
}

impl std::fmt::Display for EvalError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            EvalError::Type(err) => write!(f, "{}", err.message()),
            EvalError::Lex(err) => write!(f, "{}", err.message()),
            EvalError::Parse(err) => write!(f, "{}", err.message()),
            EvalError::StepLimit { limit } => {
                write!(f, "step limit reached ({limit})")
            }
        }
    }
}

#[derive(PartialEq, Clone, Copy)]
pub enum EvalMode {
    CallByName,
    CallByValue,
}

impl From<TypeError> for EvalError {
    fn from(err: TypeError) -> Self {
        EvalError::Type(err)
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
                let expanded = capture_avoiding_clone(global, bound);
                resolve_impl(&expanded, env, bound)
            } else {
                term.clone()
            }
        }
        Term::Lambda(param, t, body) => {
            bound.insert(param.clone());
            let body = resolve_impl(body, env, bound);
            bound.remove(param);
            Term::Lambda(param.clone(), t.clone(), Box::new(body))
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

pub fn evaluate(input: &str, eval_mode: EvalMode) -> Result<Term, EvalError> {
    let tokens = lex(input)?;
    let term = parse_term(tokens)?;

    let mut type_env = TypeEnv::new();
    let mut generator = TypeVarGenerator::new();

    seed_free_vars_term(&term, &mut type_env, &mut generator);
    infer(&term, &type_env, &mut generator)?;

    normalize(&term, eval_mode)
}

pub fn normalize(term: &Term, eval_mode: EvalMode) -> Result<Term, EvalError> {
    normalize_with_limit(term, 1_000, eval_mode)
}

pub fn normalize_with_limit(
    term: &Term,
    limit: u32,
    eval_mode: EvalMode,
) -> Result<Term, EvalError> {
    let mut current = term.clone();

    let mut steps: u32 = 0;

    while let Some(reduced) = step(&current, eval_mode) {
        if steps >= limit {
            return Err(EvalError::StepLimit { limit });
        }
        steps += 1;
        println!("{steps}: {}", &reduced);
        current = reduced;
    }

    Ok(current)
}

fn is_value(term: &Term) -> bool {
    matches!(term, Term::Var(_) | Term::Lambda(_, _, _))
}

fn step_cbn(term: &Term) -> Option<Term> {
    match term {
        Term::Var(_) => None,
        Term::Lambda(_, _, _) => None,
        Term::Let { name, value, body } => Some(substitute(body, name, value)),
        Term::Application(left, right) => match left.as_ref() {
            Term::Lambda(param, _, body) => Some(substitute(body, param, right)),
            _ => {
                step_cbn(left).map(|new_left| Term::Application(Box::new(new_left), right.clone()))
            }
        },
    }
}

fn step_cbv(term: &Term) -> Option<Term> {
    match term {
        Term::Var(_) => None,
        Term::Lambda(_, _, _) => None,
        Term::Let { name, value, body } => {
            if is_value(value) {
                Some(substitute(body, name, value))
            } else {
                step_cbv(value).map(|new_value| Term::Let {
                    name: name.clone(),
                    value: Box::new(new_value),
                    body: body.clone(),
                })
            }
        }
        Term::Application(left, right) => {
            if let Some(new_left) = step_cbv(left) {
                return Some(Term::Application(Box::new(new_left), right.clone()));
            }

            if let Term::Lambda(param, _, body) = left.as_ref() {
                if let Some(new_right) = step(right, EvalMode::CallByValue) {
                    return Some(Term::Application(left.clone(), Box::new(new_right)));
                }

                if is_value(right) {
                    return Some(substitute(body, param, right));
                }
            }

            None
        }
    }
}

fn step(term: &Term, eval_mode: EvalMode) -> Option<Term> {
    if eval_mode == EvalMode::CallByName {
        step_cbn(term)
    } else {
        step_cbv(term)
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
        Term::Lambda(param, t, body) => {
            if param == old {
                term.clone()
            } else {
                Term::Lambda(param.clone(), t.clone(), Box::new(rename(body, old, new)))
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
        Term::Lambda(param, t, body) => {
            if avoid.contains(param) {
                let mut used = avoid.clone();
                used.extend(free_vars(body));
                let fresh = create_fresh_name(param, &used);
                let renamed_body = rename(body, param, &fresh);
                let mut next_avoid = avoid.clone();
                next_avoid.insert(fresh.clone());
                Term::Lambda(
                    fresh,
                    t.clone(),
                    Box::new(capture_avoiding_clone(&renamed_body, &next_avoid)),
                )
            } else {
                let mut next_avoid = avoid.clone();
                next_avoid.insert(param.clone());
                Term::Lambda(
                    param.clone(),
                    t.clone(),
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

fn update_lambda(lambda: &Term, new: &str) -> (String, Option<Type>, Term) {
    match lambda {
        Term::Lambda(param, t, body) => {
            let renamed_body = rename(body, param, new);
            (new.to_string(), t.clone(), renamed_body)
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
        let candidate = format!("{base}{i}");
        if !used.contains(&candidate) {
            return candidate;
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
        Term::Lambda(name, _, body) => {
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
    let free_replacement = free_vars(replacement);

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
        Term::Lambda(param, t, body) => {
            let free_body = free_vars(body);
            if param == var || !free_body.contains(var) {
                term.clone()
            } else if !free_replacement.contains(param) {
                Term::Lambda(
                    param.clone(),
                    t.clone(),
                    Box::new(substitute(body, var, replacement)),
                )
            } else {
                let mut used = free_replacement;
                used.extend(free_body);
                used.insert(param.clone());
                used.insert(var.to_string());
                let fresh_name = create_fresh_name(param, &used);
                let (new_param, t, new_body) = update_lambda(term, &fresh_name);
                Term::Lambda(
                    new_param,
                    t.clone(),
                    Box::new(substitute(&new_body, var, replacement)),
                )
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
            } else if free_replacement.contains(name) {
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

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use crate::{
        eval::{
            EvalError, EvalMode, create_fresh_name, free_vars, normalize, rename, step, substitute,
            update_lambda,
        },
        lexer::lex,
        parser::{Term, parse_term},
    };

    fn term(input: &str) -> Term {
        parse_term(lex(input).unwrap()).unwrap()
    }

    fn normalize_cbv(term: &Term) -> Result<Term, EvalError> {
        normalize(term, EvalMode::CallByValue)
    }

    fn normalize_cbn(term: &Term) -> Result<Term, EvalError> {
        normalize(term, EvalMode::CallByName)
    }

    fn step_cbv(term: &Term) -> Option<Term> {
        step(term, EvalMode::CallByValue)
    }

    fn step_cbn(term: &Term) -> Option<Term> {
        step(term, EvalMode::CallByName)
    }

    mod cbv {
        use super::*;

        mod normalize {
            use super::*;

            #[test]
            fn test_normalize_respects_capture_avoidance() {
                assert_eq!(
                    normalize_cbv(&term("(\\x.\\y.x) y")).unwrap(),
                    term("\\y1.y")
                );

                assert_eq!(
                    normalize_cbv(&term("(\\x.\\y.x y) y")).unwrap(),
                    term("\\y1.y y1")
                );
            }

            #[test]
            fn test_normalize_simple() {
                assert_eq!(normalize_cbv(&term("(\\x.x) y")).unwrap(), term("y"));

                assert_eq!(
                    normalize_cbv(&term("(\\x.\\y.x) a")).unwrap(),
                    term("\\y.a")
                );

                assert_eq!(normalize_cbv(&term("(\\x.x x) y")).unwrap(), term("y y"));
            }

            #[test]
            fn test_normalize_let() {
                assert_eq!(
                    normalize_cbv(&term("let id = \\x.x in id y")).unwrap(),
                    term("y")
                );
                assert_eq!(
                    normalize_cbv(&term("let x = a in let x = b in x")).unwrap(),
                    term("b")
                );
            }

            #[test]
            fn test_normalize_multiple_steps() {
                assert_eq!(
                    normalize_cbv(&term("((\\f.f) (\\x.x)) y")).unwrap(),
                    term("y")
                );

                assert_eq!(normalize_cbv(&term("(\\x.\\y.x) a b")).unwrap(), term("a"));

                assert_eq!(
                    normalize_cbv(&term("(\\f.\\x.f x) g z")).unwrap(),
                    term("g z")
                );
            }
        }

        mod step {
            use super::*;

            #[test]
            fn test_simple_step() {
                assert_eq!(step_cbv(&term("x")), None);
                assert_eq!(step_cbv(&term("\\x.x")), None);
                assert_eq!(step_cbv(&term("(\\x.x) y")), Some(term("y")));
                assert_eq!(
                    step_cbv(&term("((\\f.f) (\\x.x)) y")),
                    Some(term("(\\x.x) y"))
                );
                assert_eq!(step_cbv(&term("f ((\\x.x) y)")), None);
            }

            #[test]
            fn test_step_stuck_terms() {
                assert_eq!(step_cbv(&term("x")), None);
                assert_eq!(step_cbv(&term("\\x.x")), None);
                assert_eq!(step_cbv(&term("f x")), None);
            }

            #[test]
            fn test_step_simple_beta() {
                assert_eq!(step_cbv(&term("(\\x.x) y")), Some(term("y")));

                assert_eq!(step_cbv(&term("(\\x.\\y.x) a")), Some(term("\\y.a")));

                assert_eq!(step_cbv(&term("(\\x.x x) y")), Some(term("y y")));
            }

            #[test]
            fn test_step_reduces_left_side_of_application() {
                assert_eq!(
                    step_cbv(&term("((\\f.f) (\\x.x)) y")),
                    Some(term("(\\x.x) y"))
                );

                assert_eq!(step_cbv(&term("(((\\f.f) g) z)")), Some(term("(g z)")));
            }

            #[test]
            fn test_step_call_by_value_reduces_argument() {
                assert_eq!(step_cbv(&term("f ((\\x.x) y)")), None);

                assert_eq!(
                    step_cbv(&term("(\\x.z) ((\\y.y) w)")),
                    Some(term("(\\x.z) w"))
                );
            }
        }
    }

    mod cbn {
        use super::*;

        mod normalize {
            use super::*;

            #[test]
            fn test_normalize_under_call_by_name() {
                assert_eq!(
                    normalize_cbn(&term("(\\x.z) ((\\y.y) w)")).unwrap(),
                    term("z")
                );

                assert_eq!(
                    normalize_cbn(&term("(\\x.x) ((\\y.y) z)")).unwrap(),
                    term("z")
                );
            }
        }

        mod step {
            use super::*;

            #[test]
            fn test_step_cbn_does_not_reduce_argument() {
                assert_eq!(step_cbn(&term("f ((\\x.x) y)")), None);

                assert_eq!(step_cbn(&term("(\\x.z) ((\\y.y) w)")), Some(term("z")));
            }

            #[test]
            fn test_step_cbn_let_is_lazy() {
                assert_eq!(step_cbn(&term("let x = ((\\y.y) w) in z")), Some(term("z")));
            }
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
                        None,
                        Box::new(Term::Application(
                            Box::new(Term::Var("x".into())),
                            Box::new(Term::Var("y".into()))
                        ))
                    ),
                    "y1",
                ),
                (
                    "y1".into(),
                    None,
                    Term::Application(
                        Box::new(Term::Var("x".into())),
                        Box::new(Term::Var("y1".into()))
                    )
                )
            );
            assert_eq!(
                update_lambda(
                    &Term::Lambda(
                        "y".into(),
                        None,
                        Box::new(Term::Lambda(
                            "y".into(),
                            None,
                            Box::new(Term::Var("y".into()))
                        ))
                    ),
                    "y1",
                ),
                (
                    "y1".into(),
                    None,
                    Term::Lambda("y".into(), None, Box::new(Term::Var("y".into())))
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
                    &Term::Lambda("y".into(), None, Box::new(Term::Var("y".into()))),
                    "y",
                    "z"
                ),
                Term::Lambda("y".into(), None, Box::new(Term::Var("y".into())))
            );
            assert_eq!(
                rename(
                    &Term::Lambda(
                        "z".into(),
                        None,
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
                    None,
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
            assert_eq!(substitute(&term("x"), "x", &term("y")), term("y"));
            assert_eq!(substitute(&term("z"), "x", &term("y")), term("z"));

            assert_eq!(substitute(&term("x z"), "x", &term("y")), term("y z"));

            assert_eq!(substitute(&term("x x"), "x", &term("y")), term("y y"));

            assert_eq!(substitute(&term("x y"), "x", &term("f z")), term("(f z) y"));

            assert_eq!(
                substitute(&term("f x"), "x", &term("\\z.z")),
                term("f (\\z.z)")
            );
        }

        #[test]
        fn test_substitute_shadowing() {
            assert_eq!(substitute(&term("\\x.x"), "x", &term("y")), term("\\x.x"));

            assert_eq!(
                substitute(&term("\\x.x z"), "x", &term("y")),
                term("\\x.x z")
            );

            assert_eq!(substitute(&term("\\z.x"), "x", &term("y")), term("\\z.y"));

            assert_eq!(
                substitute(&term("\\z.x z"), "x", &term("y")),
                term("\\z.y z")
            );

            assert_eq!(
                substitute(&term("\\z.(\\x.x) x"), "x", &term("y")),
                term("\\z.(\\x.x) y")
            );
        }

        #[test]
        fn test_substitute_capture_avoidance_simple() {
            assert_eq!(substitute(&term("\\y.x"), "x", &term("y1")), term("\\y.y1"));

            assert_eq!(substitute(&term("\\y.x"), "x", &term("y")), term("\\y1.y"));

            assert_eq!(
                substitute(&term("\\y.x y"), "x", &term("y")),
                term("\\y1.y y1")
            );

            assert_eq!(
                substitute(&term("\\y.y x"), "x", &term("y")),
                term("\\y1.y1 y")
            );

            assert_eq!(
                substitute(&term("\\y.f x y"), "x", &term("y")),
                term("\\y1.f y y1")
            );
        }

        #[test]
        fn test_substitute_capture_avoidance_with_compound_replacement() {
            assert_eq!(
                substitute(&term("\\y.x"), "x", &term("y z")),
                term("\\y1.y z")
            );

            assert_eq!(
                substitute(&term("\\y.x y"), "x", &term("y z")),
                term("\\y1.(y z) y1")
            );

            assert_eq!(
                substitute(&term("\\y.f (x y)"), "x", &term("y z")),
                term("\\y1.f ((y z) y1)")
            );

            assert_eq!(
                substitute(&term("\\y.x"), "x", &term("\\z.y z")),
                term("\\y1.\\z.y z")
            );

            assert_eq!(
                substitute(&term("\\y.x y"), "x", &term("\\z.y z")),
                term("\\y1.(\\z.y z) y1")
            );
        }

        #[test]
        fn test_substitute_nested_lambdas() {
            assert_eq!(
                substitute(&term("\\a.\\b.x a b"), "x", &term("y")),
                term("\\a.\\b.y a b")
            );

            assert_eq!(
                substitute(&term("\\a.\\b.x a b"), "x", &term("a")),
                term("\\a1.\\b.a a1 b")
            );

            assert_eq!(
                substitute(&term("\\y.\\z.x y z"), "x", &term("y")),
                term("\\y1.\\z.y y1 z")
            );

            assert_eq!(
                substitute(&term("\\z.\\y.x y"), "x", &term("y")),
                term("\\z.\\y1.y y1")
            );

            assert_eq!(
                substitute(&term("\\z.\\y.x z y"), "x", &term("y")),
                term("\\z.\\y1.y z y1")
            );
        }

        #[test]
        fn test_substitute_nested_shadowing() {
            assert_eq!(
                substitute(&term("\\y.\\y.y"), "x", &term("z")),
                term("\\y.\\y.y")
            );

            assert_eq!(
                substitute(&term("\\y.\\x.x"), "x", &term("z")),
                term("\\y.\\x.x")
            );

            assert_eq!(
                substitute(&term("\\y.\\x.y x"), "x", &term("z")),
                term("\\y.\\x.y x")
            );

            assert_eq!(
                substitute(&term("\\z.(\\x.x) x"), "x", &term("y")),
                term("\\z.(\\x.x) y")
            );

            assert_eq!(
                substitute(&term("\\z.(\\x.x z) x"), "x", &term("y")),
                term("\\z.(\\x.x z) y")
            );
        }

        #[test]
        fn test_substitute_more_complex_applications() {
            assert_eq!(
                substitute(&term("(x y) z"), "x", &term("f")),
                term("(f y) z")
            );

            assert_eq!(
                substitute(&term("f (x y)"), "x", &term("z")),
                term("f (z y)")
            );

            assert_eq!(
                substitute(&term("(x x) (x x)"), "x", &term("y")),
                term("(y y) (y y)")
            );

            assert_eq!(
                substitute(&term("(\\z.z) x"), "x", &term("y")),
                term("(\\z.z) y")
            );

            assert_eq!(
                substitute(&term("x (\\z.x z)"), "x", &term("y")),
                term("y (\\z.y z)")
            );
        }

        #[test]
        fn test_substitute_replacement_with_free_vars() {
            assert_eq!(
                substitute(&term("\\a.x"), "x", &term("y a")),
                term("\\a1.y a")
            );

            assert_eq!(
                substitute(&term("\\a.x a"), "x", &term("y a")),
                term("\\a1.(y a) a1")
            );

            assert_eq!(
                substitute(&term("\\b.\\a.x b a"), "x", &term("a")),
                term("\\b.\\a1.a b a1")
            );

            assert_eq!(
                substitute(&term("\\b.\\a.x a b"), "x", &term("a")),
                term("\\b.\\a1.a a1 b")
            );
        }

        #[test]
        fn test_substitute_identity_like_cases() {
            assert_eq!(
                substitute(&term("\\x.\\y.x"), "x", &term("z")),
                term("\\x.\\y.x")
            );

            assert_eq!(
                substitute(&term("\\z.\\y.z"), "x", &term("y")),
                term("\\z.\\y.z")
            );

            assert_eq!(
                substitute(&term("\\z.\\y.x"), "x", &term("y")),
                term("\\z.\\y1.y")
            );

            assert_eq!(
                substitute(&term("\\z.\\y.x z"), "x", &term("y")),
                term("\\z.\\y1.y z")
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
                    &Term::Lambda("z".into(), None, Box::new(Term::Var("x".into())),),
                    "x",
                    &Term::Var("y".into())
                ),
                Term::Lambda("z".into(), None, Box::new(Term::Var("y".into())),),
            );

            assert_eq!(
                substitute(
                    &Term::Lambda("x".into(), None, Box::new(Term::Var("x".into())),),
                    "x",
                    &Term::Var("y".into())
                ),
                Term::Lambda("x".into(), None, Box::new(Term::Var("x".into())),),
            );
        }
    }

    mod test_free_vars {
        use super::*;

        #[test]
        fn test_free_vars() {
            assert_eq!(free_vars(&term("x")), HashSet::from(["x".into()]));
            assert_eq!(free_vars(&term("\\x.x")), HashSet::new());
            assert_eq!(free_vars(&term("\\x.y")), HashSet::from(["y".into()]));
            assert_eq!(
                free_vars(&term("f x")),
                HashSet::from(["f".into(), "x".into()])
            );
            assert_eq!(free_vars(&term("\\x.x y")), HashSet::from(["y".into()]));
            assert_eq!(free_vars(&term("(\\x.x) y")), HashSet::from(["y".into()]));
            assert_eq!(free_vars(&term("\\x.\\y.x z")), HashSet::from(["z".into()]))
        }
    }
}
