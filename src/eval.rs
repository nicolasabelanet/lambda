use std::collections::HashSet;

use crate::{
    lexer::{lex, LexError},
    parser::{parse, ParseError, Term},
};

#[derive(Debug)]
pub enum EvalError {
    Lex(LexError),
    Parse(ParseError),
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

pub fn evaluate(input: &str) -> Result<Term, EvalError> {
    let tokens = lex(input)?;
    let ast = parse(tokens)?;
    Ok(normalize(&ast))
}

pub fn normalize(term: &Term) -> Term {
    normalize_with_limit(term, 1_000)
}

pub fn normalize_with_limit(term: &Term, limit: u32) -> Term {
    let mut current = term.clone();

    let mut steps: u32 = 0;

    while let Some(reduced) = step(&current) {
        if steps >= limit {
            panic!("Too many steps")
        }
        steps += 1;
        current = reduced;
    }

    current
}

fn step(term: &Term) -> Option<Term> {
    match term {
        Term::Var(_) => None,
        Term::Lambda(_, _) => None,
        Term::Application(left, right) => match left.as_ref() {
            Term::Lambda(param, body) => Some(substitute(body, param, right)),
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
    }
}

fn update_lambda(lambda: &Term, new: &str) -> Term {
    match lambda {
        Term::Lambda(param, body) => {
            let renamed_body = rename(body, param, new);
            Term::Lambda(new.to_string(), Box::new(renamed_body))
        }
        _ => panic!("update_lambda called on non lambda"),
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

pub fn free_vars(term: &Term) -> HashSet<String> {
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
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use crate::{
        eval::{create_fresh_name, free_vars, normalize, rename, step, substitute, update_lambda},
        lexer::lex,
        parser::{parse, Term},
    };

    fn ast(input: &str) -> Term {
        parse(lex(input).unwrap()).unwrap()
    }

    mod test_normalize {

        use super::*;
        #[test]
        fn test_normalize_respects_capture_avoidance() {
            assert_eq!(normalize(&ast("(\\x.\\y.x) y")), ast("\\y1.y"));

            assert_eq!(normalize(&ast("(\\x.\\y.x y) y")), ast("\\y1.y y1"));
        }

        #[test]
        fn test_normalize_simple() {
            assert_eq!(normalize(&ast("(\\x.x) y")), ast("y"));

            assert_eq!(normalize(&ast("(\\x.\\y.x) a")), ast("\\y.a"));

            assert_eq!(normalize(&ast("(\\x.x x) y")), ast("y y"));
        }

        #[test]
        fn test_normalize_multiple_steps() {
            assert_eq!(normalize(&ast("((\\f.f) (\\x.x)) y")), ast("y"));

            assert_eq!(normalize(&ast("(\\x.\\y.x) a b")), ast("a"));

            assert_eq!(normalize(&ast("(\\f.\\x.f x) g z")), ast("g z"));
        }

        #[test]
        fn test_normalize_under_call_by_name() {
            assert_eq!(normalize(&ast("(\\x.z) ((\\y.y) w)")), ast("z"));

            assert_eq!(normalize(&ast("(\\x.x) ((\\y.y) z)")), ast("z"));
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

            assert_eq!(step(&ast("(\\x.z) ((\\y.y) w)")), Some(ast("z")));
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
