use std::collections::{HashMap, HashSet};

use crate::{
    lexer::{lex, LexError},
    parser::{parse_term, ParseError, Term},
    typing::{infer, seed_free_vars_term, Type, TypeEnv, TypeError, TypeVarGenerator},
};

#[derive(Debug)]
pub enum EvalError {
    Lex(LexError),
    Type(TypeError),
    Parse(ParseError),
    StepLimit { limit: u32 },
}

impl std::fmt::Display for EvalError {
    /// Formats evaluation errors for display.
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
    /// Converts type errors into evaluation errors.
    fn from(err: TypeError) -> Self {
        EvalError::Type(err)
    }
}

impl From<LexError> for EvalError {
    /// Converts lex errors into evaluation errors.
    fn from(err: LexError) -> Self {
        EvalError::Lex(err)
    }
}

impl From<ParseError> for EvalError {
    /// Converts parse errors into evaluation errors.
    fn from(err: ParseError) -> Self {
        EvalError::Parse(err)
    }
}

/// Resolves global variables in a term using the environment.
pub fn resolve(term: &Term, env: &HashMap<String, Term>) -> Term {
    let mut bound = HashSet::new();
    resolve_impl(term, env, &mut bound)
}

/// Resolves globals while tracking locally bound variables.
fn resolve_impl(term: &Term, env: &HashMap<String, Term>, bound: &mut HashSet<String>) -> Term {
    match term {
        Term::Var(name, _) => {
            if bound.contains(name) {
                term.clone()
            } else if let Some(global) = env.get(name) {
                let expanded = capture_avoiding_clone(global, bound);
                resolve_impl(&expanded, env, bound)
            } else {
                term.clone()
            }
        }
        Term::Lambda(param, t, body, span) => {
            bound.insert(param.clone());
            let body = resolve_impl(body, env, bound);
            bound.remove(param);
            Term::Lambda(param.clone(), t.clone(), Box::new(body), span.clone())
        }
        Term::Application(left, right, span) => {
            let left = resolve_impl(left, env, bound);
            let right = resolve_impl(right, env, bound);
            Term::Application(Box::new(left), Box::new(right), span.clone())
        }
        Term::Let {
            name,
            value,
            body,
            span,
        } => {
            let value = resolve_impl(value, env, bound);
            bound.insert(name.clone());
            let body = resolve_impl(body, env, bound);
            bound.remove(name);
            Term::Let {
                name: name.clone(),
                value: Box::new(value),
                body: Box::new(body),
                span: span.clone(),
            }
        }
    }
}

/// Parses and evaluates an input string.
pub fn evaluate(input: &str, eval_mode: EvalMode) -> Result<Term, EvalError> {
    let tokens = lex(input)?;
    let term = parse_term(tokens)?;

    let mut type_env = TypeEnv::new();
    let mut generator = TypeVarGenerator::new();

    seed_free_vars_term(&term, &mut type_env, &mut generator);
    infer(&term, &type_env, &mut generator)?;

    normalize(&term, eval_mode)
}

/// Normalizes a term using the requested evaluation mode.
pub fn normalize(term: &Term, eval_mode: EvalMode) -> Result<Term, EvalError> {
    normalize_with_limit(term, 1_000, eval_mode)
}

/// Normalizes a term with a maximum step limit.
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

/// Returns true if the term is a value.
fn is_value(term: &Term) -> bool {
    matches!(term, Term::Var(_, _) | Term::Lambda(_, _, _, _))
}

/// Performs one call-by-name reduction step.
fn step_cbn(term: &Term) -> Option<Term> {
    match term {
        Term::Var(_, _) => None,
        Term::Lambda(_, _, _, _) => None,
        Term::Let {
            name,
            value,
            body,
            span,
        } => Some(substitute(body, name, value).with_span(span.clone())),
        Term::Application(left, right, span) => match left.as_ref() {
            Term::Lambda(param, _, body, _) => {
                Some(substitute(body, param, right).with_span(span.clone()))
            }
            _ => step_cbn(left)
                .map(|new_left| Term::Application(Box::new(new_left), right.clone(), span.clone())),
        },
    }
}

/// Performs one call-by-value reduction step.
fn step_cbv(term: &Term) -> Option<Term> {
    match term {
        Term::Var(_, _) => None,
        Term::Lambda(_, _, _, _) => None,
        Term::Let {
            name,
            value,
            body,
            span,
        } => {
            if is_value(value) {
                Some(substitute(body, name, value).with_span(span.clone()))
            } else {
                step_cbv(value).map(|new_value| Term::Let {
                    name: name.clone(),
                    value: Box::new(new_value),
                    body: body.clone(),
                    span: span.clone(),
                })
            }
        }
        Term::Application(left, right, span) => {
            if let Some(new_left) = step_cbv(left) {
                return Some(Term::Application(
                    Box::new(new_left),
                    right.clone(),
                    span.clone(),
                ));
            }

            if let Term::Lambda(param, _, body, _) = left.as_ref() {
                if let Some(new_right) = step(right, EvalMode::CallByValue) {
                    return Some(Term::Application(
                        left.clone(),
                        Box::new(new_right),
                        span.clone(),
                    ));
                }

                if is_value(right) {
                    return Some(substitute(body, param, right).with_span(span.clone()));
                }
            }

            None
        }
    }
}

/// Performs one reduction step according to the mode.
fn step(term: &Term, eval_mode: EvalMode) -> Option<Term> {
    if eval_mode == EvalMode::CallByName {
        step_cbn(term)
    } else {
        step_cbv(term)
    }
}

/// Renames bound variables in a term.
fn rename(term: &Term, old: &str, new: &str) -> Term {
    match term {
        Term::Var(name, span) => {
            if name == old {
                Term::Var(new.to_string(), span.clone())
            } else {
                term.clone()
            }
        }
        Term::Application(left, right, span) => Term::Application(
            Box::new(rename(left, old, new)),
            Box::new(rename(right, old, new)),
            span.clone(),
        ),
        Term::Lambda(param, t, body, span) => {
            if param == old {
                term.clone()
            } else {
                Term::Lambda(
                    param.clone(),
                    t.clone(),
                    Box::new(rename(body, old, new)),
                    span.clone(),
                )
            }
        }
        Term::Let {
            name,
            value,
            body,
            span,
        } => {
            if name == old {
                Term::Let {
                    name: name.clone(),
                    value: Box::new(rename(value, old, new)),
                    body: body.clone(),
                    span: span.clone(),
                }
            } else {
                Term::Let {
                    name: name.clone(),
                    value: Box::new(rename(value, old, new)),
                    body: Box::new(rename(body, old, new)),
                    span: span.clone(),
                }
            }
        }
    }
}

/// Clones a term while avoiding variable capture.
fn capture_avoiding_clone(term: &Term, avoid: &HashSet<String>) -> Term {
    match term {
        Term::Var(_, _) => term.clone(),
        Term::Application(left, right, span) => Term::Application(
            Box::new(capture_avoiding_clone(left, avoid)),
            Box::new(capture_avoiding_clone(right, avoid)),
            span.clone(),
        ),
        Term::Lambda(param, t, body, span) => {
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
                    span.clone(),
                )
            } else {
                let mut next_avoid = avoid.clone();
                next_avoid.insert(param.clone());
                Term::Lambda(
                    param.clone(),
                    t.clone(),
                    Box::new(capture_avoiding_clone(body, &next_avoid)),
                    span.clone(),
                )
            }
        }
        Term::Let {
            name,
            value,
            body,
            span,
        } => {
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
                    span: span.clone(),
                }
            } else {
                let mut next_avoid = avoid.clone();
                next_avoid.insert(name.clone());
                Term::Let {
                    name: name.clone(),
                    value: Box::new(new_value),
                    body: Box::new(capture_avoiding_clone(body, &next_avoid)),
                    span: span.clone(),
                }
            }
        }
    }
}

/// Renames a lambda parameter and returns updated components.
fn update_lambda(lambda: &Term, new: &str) -> (String, Option<Type>, Term, crate::lexer::Span) {
    match lambda {
        Term::Lambda(param, t, body, span) => {
            let renamed_body = rename(body, param, new);
            (new.to_string(), t.clone(), renamed_body, span.clone())
        }
        _ => unreachable!("update_lambda called on non lambda"),
    }
}

/// Creates a fresh name not in the used set.
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

/// Collects free variables in a term.
fn free_vars(term: &Term) -> HashSet<String> {
    match term {
        Term::Var(name, _) => HashSet::from([name.clone()]),
        Term::Application(left, right, _) => {
            let mut vars = free_vars(left);
            vars.extend(free_vars(right));
            vars
        }
        Term::Lambda(name, _, body, _) => {
            let mut body_vars = free_vars(body);
            body_vars.remove(name);
            body_vars
        }
        Term::Let {
            name, value, body, ..
        } => {
            let mut vars = free_vars(value);
            let mut body_vars = free_vars(body);
            body_vars.remove(name);
            vars.extend(body_vars);
            vars
        }
    }
}

/// Substitutes a variable with a term, avoiding capture.
pub fn substitute(term: &Term, var: &str, replacement: &Term) -> Term {
    let free_replacement = free_vars(replacement);

    match term {
        Term::Var(name, _) => {
            if name == var {
                replacement.clone()
            } else {
                term.clone()
            }
        }
        Term::Application(left, right, span) => {
            let new_left = substitute(left, var, replacement);
            let new_right = substitute(right, var, replacement);
            Term::Application(Box::new(new_left), Box::new(new_right), span.clone())
        }
        Term::Lambda(param, t, body, span) => {
            let free_body = free_vars(body);
            if param == var || !free_body.contains(var) {
                term.clone()
            } else if !free_replacement.contains(param) {
                Term::Lambda(
                    param.clone(),
                    t.clone(),
                    Box::new(substitute(body, var, replacement)),
                    span.clone(),
                )
            } else {
                let mut used = free_replacement;
                used.extend(free_body);
                used.insert(param.clone());
                used.insert(var.to_string());
                let fresh_name = create_fresh_name(param, &used);
                let (new_param, t, new_body, lambda_span) = update_lambda(term, &fresh_name);
                Term::Lambda(
                    new_param,
                    t.clone(),
                    Box::new(substitute(&new_body, var, replacement)),
                    lambda_span,
                )
            }
        }
        Term::Let {
            name,
            value,
            body,
            span,
        } => {
            let new_value = substitute(value, var, replacement);
            if name == var {
                Term::Let {
                    name: name.clone(),
                    value: Box::new(new_value),
                    body: body.clone(),
                    span: span.clone(),
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
                    span: span.clone(),
                }
            } else {
                Term::Let {
                    name: name.clone(),
                    value: Box::new(new_value),
                    body: Box::new(substitute(body, var, replacement)),
                    span: span.clone(),
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
            EvalError, EvalMode,
        },
        lexer::{lex, Span},
        parser::{parse_term, Term},
    };

    /// Parses a term for tests.
    fn term(input: &str) -> Term {
        parse_term(lex(input).unwrap()).unwrap()
    }

    /// Returns a dummy span for tests.
    fn span() -> Span {
        Span { start: 0, end: 0 }
    }

    /// Builds a variable term with a dummy span.
    fn var(name: &str) -> Term {
        Term::Var(name.into(), span())
    }

    /// Builds a lambda term with a dummy span.
    fn lam(name: &str, body: Term) -> Term {
        Term::Lambda(name.into(), None, Box::new(body), span())
    }

    /// Builds an application term with a dummy span.
    fn app(left: Term, right: Term) -> Term {
        Term::Application(Box::new(left), Box::new(right), span())
    }

    /// Normalizes a term using call-by-value.
    fn normalize_cbv(term: &Term) -> Result<Term, EvalError> {
        normalize(term, EvalMode::CallByValue)
    }

    /// Normalizes a term using call-by-name.
    fn normalize_cbn(term: &Term) -> Result<Term, EvalError> {
        normalize(term, EvalMode::CallByName)
    }

    /// Performs a single call-by-value step.
    fn step_cbv(term: &Term) -> Option<Term> {
        step(term, EvalMode::CallByValue)
    }

    /// Performs a single call-by-name step.
    fn step_cbn(term: &Term) -> Option<Term> {
        step(term, EvalMode::CallByName)
    }

    mod cbv {
        use super::*;

        mod normalize {
            use super::*;

            #[test]
            /// Ensures normalization avoids variable capture.
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
            /// Ensures normalization reduces simple terms.
            fn test_normalize_simple() {
                assert_eq!(normalize_cbv(&term("(\\x.x) y")).unwrap(), term("y"));

                assert_eq!(
                    normalize_cbv(&term("(\\x.\\y.x) a")).unwrap(),
                    term("\\y.a")
                );

                assert_eq!(normalize_cbv(&term("(\\x.x x) y")).unwrap(), term("y y"));
            }

            #[test]
            /// Ensures let normalization works.
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
            /// Ensures normalization performs multiple steps.
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
            /// Ensures a basic step reduces as expected.
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
            /// Ensures stuck terms do not reduce.
            fn test_step_stuck_terms() {
                assert_eq!(step_cbv(&term("x")), None);
                assert_eq!(step_cbv(&term("\\x.x")), None);
                assert_eq!(step_cbv(&term("f x")), None);
            }

            #[test]
            /// Ensures beta-reduction steps work.
            fn test_step_simple_beta() {
                assert_eq!(step_cbv(&term("(\\x.x) y")), Some(term("y")));

                assert_eq!(step_cbv(&term("(\\x.\\y.x) a")), Some(term("\\y.a")));

                assert_eq!(step_cbv(&term("(\\x.x x) y")), Some(term("y y")));
            }

            #[test]
            /// Ensures reduction proceeds on the left side.
            fn test_step_reduces_left_side_of_application() {
                assert_eq!(
                    step_cbv(&term("((\\f.f) (\\x.x)) y")),
                    Some(term("(\\x.x) y"))
                );

                assert_eq!(step_cbv(&term("(((\\f.f) g) z)")), Some(term("(g z)")));
            }

            #[test]
            /// Ensures call-by-value reduces arguments.
            fn test_step_call_by_value_reduces_argument() {
                assert_eq!(step_cbv(&term("f ((\\x.x) y)")), None);

                assert_eq!(
                    step_cbv(&term("(\\x.z) ((\\y.y) w)")),
                    Some(term("(\\x.z) w"))
                );
            }

            #[test]
            /// Ensures reductions preserve the application redex span.
            fn test_step_preserves_redex_span_application() {
                let input = term("(\\x.x) y");
                let redex_span = input.span();
                let reduced = step_cbv(&input).expect("expected a reduction");
                assert_eq!(reduced.span().start, redex_span.start);
                assert_eq!(reduced.span().end, redex_span.end);
            }

            #[test]
            /// Ensures reductions preserve the let redex span.
            fn test_step_preserves_redex_span_let() {
                let input = term("let x = y in z");
                let redex_span = input.span();
                let reduced = step_cbv(&input).expect("expected a reduction");
                assert_eq!(reduced.span().start, redex_span.start);
                assert_eq!(reduced.span().end, redex_span.end);
            }
        }
    }

    mod cbn {
        use super::*;

        mod normalize {
            use super::*;

            #[test]
            /// Ensures call-by-name normalization behaves correctly.
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
            /// Ensures call-by-name does not reduce arguments.
            fn test_step_cbn_does_not_reduce_argument() {
                assert_eq!(step_cbn(&term("f ((\\x.x) y)")), None);

                assert_eq!(step_cbn(&term("(\\x.z) ((\\y.y) w)")), Some(term("z")));
            }

            #[test]
            /// Ensures call-by-name treats let bindings lazily.
            fn test_step_cbn_let_is_lazy() {
                assert_eq!(step_cbn(&term("let x = ((\\y.y) w) in z")), Some(term("z")));
            }
        }
    }

    mod test_update_lambda {
        use super::*;

        #[test]
        /// Ensures update_lambda renames parameters correctly.
        fn test_simple() {
            assert_eq!(
                update_lambda(&lam("y", app(var("x"), var("y"))), "y1",),
                ("y1".into(), None, app(var("x"), var("y1")), span(),)
            );
            assert_eq!(
                update_lambda(&lam("y", lam("y", var("y"))), "y1",),
                ("y1".into(), None, lam("y", var("y")), span(),),
            );
        }
    }

    mod test_rename {
        use super::*;

        #[test]
        /// Ensures rename behaves correctly for basic cases.
        fn test_simple() {
            assert_eq!(
                rename(&app(var("x"), var("y")), "y", "y1"),
                app(var("x"), var("y1"))
            );
            assert_eq!(rename(&lam("y", var("y")), "y", "z"), lam("y", var("y")));
            assert_eq!(
                rename(&lam("z", app(var("y"), var("z"))), "y", "y1"),
                lam("z", app(var("y1"), var("z")))
            );
        }
    }

    mod test_fresh_name {
        use super::*;

        #[test]
        /// Ensures fresh name generation skips used names.
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
        /// Ensures basic substitution works.
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
        /// Ensures substitution respects shadowing.
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
        /// Ensures substitution avoids capture in simple cases.
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
        /// Ensures substitution avoids capture with compound replacements.
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
        /// Ensures substitution works under nested lambdas.
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
        /// Ensures substitution respects nested shadowing.
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
        /// Ensures substitution works in complex applications.
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
        /// Ensures substitution handles replacements with free vars.
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
        /// Ensures substitution leaves identity-like cases unchanged.
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
        /// Ensures basic substitution results for variables.
        fn test_naive_substitution() {
            assert_eq!(substitute(&var("x"), "x", &var("y")), var("y"));
            assert_eq!(substitute(&var("z"), "x", &var("y")), var("z"));
            assert_eq!(
                substitute(&app(var("x"), var("z")), "x", &var("y")),
                app(var("y"), var("z"))
            );
            assert_eq!(
                substitute(&lam("z", var("x")), "x", &var("y")),
                lam("z", var("y"))
            );

            assert_eq!(
                substitute(&lam("x", var("x")), "x", &var("y")),
                lam("x", var("x"))
            );
        }
    }

    mod test_free_vars {
        use super::*;

        #[test]
        /// Ensures free variable collection works.
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
