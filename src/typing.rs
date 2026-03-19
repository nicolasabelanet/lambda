use std::{
    collections::{HashMap, HashSet},
    fmt::Display,
};

use crate::{
    lexer::Span,
    parser::{Statement, Term},
};

#[derive(Debug, PartialEq, Clone)]
pub enum TypeError {
    UnboundVar(String),
    OccursCheckFailed { var: TypeVar, ty: Type },
    TypeMismatch {
        expected: Type,
        found: Type,
        context: Option<TypeMismatchContext>,
    },
    ExpectedFunction { found: Type },
}
impl Display for TypeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TypeError::UnboundVar(name) => write!(f, "unbound variable '{name}'"),
            TypeError::OccursCheckFailed { var, ty } => {
                write!(f, "occurs check failed: t{var} occurs in {ty}")
            }
            TypeError::TypeMismatch {
                expected,
                found,
                context,
            } => {
                write!(f, "type mismatch: expected {expected}, found {found}")?;
                if let Some(context) = context {
                    write!(f, "\n  in: {} : {}", context.term, context.ty)?;
                }
                Ok(())
            }
            TypeError::ExpectedFunction { found } => {
                write!(f, "expected function type, found {found}")
            }
        }
    }
}

#[derive(Debug, PartialEq, Clone)]
pub struct TypeMismatchContext {
    pub term: String,
    pub ty: Type,
    pub span: Span,
}

impl TypeError {
    pub fn message(&self) -> String {
        self.to_string()
    }

    fn with_context(self, term: &Term, ty: &Type) -> TypeError {
        match self {
            TypeError::TypeMismatch {
                expected,
                found,
                ..
            } => TypeError::TypeMismatch {
                expected,
                found,
                context: Some(TypeMismatchContext {
                    term: term.to_string(),
                    ty: ty.clone(),
                    span: term.span(),
                }),
            },
            other => other,
        }
    }
}

#[derive(Debug, PartialEq, Clone)]
pub enum Type {
    Var(String),
    Arrow(Box<Type>, Box<Type>),
    Meta(TypeVar),
}

impl Display for Type {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            Type::Var(name) => write!(f, "{name}"),
            Type::Arrow(left, right) => {
                // Parenthesize left if it's also an arrow.
                match **left {
                    Type::Arrow(_, _) => write!(f, "({left}) -> {right}"),
                    _ => write!(f, "{left} -> {right}"),
                }
            }
            Type::Meta(id) => {
                write!(f, "t{id}")
            }
        }
    }
}

pub type TypeEnv = HashMap<String, TypeScheme>;

type Subst = HashMap<TypeVar, Type>;

pub type TypeVar = u32;

#[derive(Clone)]
pub struct TypeScheme {
    pub vars: Vec<TypeVar>,
    pub ty: Type,
}

pub struct TypeVarGenerator {
    next: TypeVar,
}

impl TypeVarGenerator {
    pub fn new() -> TypeVarGenerator {
        TypeVarGenerator { next: 0u32 }
    }

    fn fresh(&mut self) -> Type {
        let fresh_type = Type::Meta(self.next);
        self.next += 1;
        fresh_type
    }
}

impl Default for TypeVarGenerator {
    fn default() -> Self {
        Self::new()
    }
}

fn free_type_vars_type(ty: &Type) -> HashSet<TypeVar> {
    match ty {
        Type::Var(_) => HashSet::new(),
        Type::Meta(id) => HashSet::from_iter([*id]),
        Type::Arrow(left, right) => {
            let mut free_vars = free_type_vars_type(left);
            free_vars.extend(free_type_vars_type(right));
            free_vars
        }
    }
}

fn free_type_vars_scheme(scheme: &TypeScheme) -> HashSet<TypeVar> {
    let mut free_vars = free_type_vars_type(&scheme.ty);

    for var in scheme.vars.iter() {
        free_vars.remove(var);
    }

    free_vars
}

fn free_type_vars_env(env: &TypeEnv) -> HashSet<TypeVar> {
    let mut free_vars = HashSet::new();

    for scheme in env.values() {
        free_vars.extend(free_type_vars_scheme(scheme));
    }

    free_vars
}

fn generalize(ty: &Type, env: &TypeEnv) -> TypeScheme {
    let free_ty_vars = free_type_vars_type(ty);
    let free_env_vars = free_type_vars_env(env);

    TypeScheme {
        vars: free_ty_vars.difference(&free_env_vars).copied().collect(),
        ty: ty.clone(),
    }
}

fn instantiate(scheme: &TypeScheme, generator: &mut TypeVarGenerator) -> Type {
    let mapping = HashMap::from_iter(scheme.vars.iter().map(|var| (*var, generator.fresh())));
    replace_meta(&scheme.ty, &mapping)
}

fn replace_meta(ty: &Type, mapping: &HashMap<TypeVar, Type>) -> Type {
    match ty {
        Type::Var(_) => ty.clone(),
        Type::Meta(id) => {
            if let Some(mapped_type) = mapping.get(id) {
                mapped_type.clone()
            } else {
                ty.clone()
            }
        }
        Type::Arrow(left, right) => {
            let mapped_left = replace_meta(left, mapping);
            let mapped_right = replace_meta(right, mapping);

            Type::Arrow(Box::new(mapped_left), Box::new(mapped_right))
        }
    }
}

fn apply_subst_type(ty: &Type, subst: &Subst) -> Type {
    match ty {
        Type::Var(_) => ty.clone(),
        Type::Meta(id) => {
            if let Some(mapped_type) = subst.get(id) {
                apply_subst_type(mapped_type, subst)
            } else {
                ty.clone()
            }
        }
        Type::Arrow(left, right) => {
            let subst_left = apply_subst_type(left, subst);
            let subst_right = apply_subst_type(right, subst);

            Type::Arrow(Box::new(subst_left), Box::new(subst_right))
        }
    }
}

fn apply_subst_scheme(scheme: &TypeScheme, subst: &Subst) -> TypeScheme {
    let mut filtered_subst = subst.clone();

    for var in scheme.vars.iter() {
        filtered_subst.remove(var);
    }

    let subst_ty = apply_subst_type(&scheme.ty, &filtered_subst);

    TypeScheme {
        vars: scheme.vars.clone(),
        ty: subst_ty,
    }
}

fn apply_subst_env(env: &TypeEnv, subst: &Subst) -> TypeEnv {
    env.iter()
        .map(|(key, scheme)| (key.clone(), apply_subst_scheme(scheme, subst)))
        .collect()
}

fn compose_subst(s1: &Subst, s2: &Subst) -> Subst {
    let mut combined_subst: HashMap<_, _> = s2
        .iter()
        .map(|(key, mapping)| (*key, apply_subst_type(mapping, s1)))
        .collect();

    for (key, mapping) in s1.iter() {
        combined_subst
            .entry(*key)
            .or_insert_with(|| mapping.clone());
    }

    combined_subst
}

fn occurs(var: TypeVar, ty: &Type) -> bool {
    match ty {
        Type::Var(_) => false,
        Type::Meta(id) => var == *id,
        Type::Arrow(left, right) => occurs(var, left) || occurs(var, right),
    }
}

fn unify_var(var: TypeVar, ty: &Type, subst: &Subst) -> Result<Subst, TypeError> {
    let ty = apply_subst_type(ty, subst);

    if let Type::Meta(id) = ty
        && id == var
    {
        return Ok(subst.clone());
    }

    if occurs(var, &ty) {
        return Err(TypeError::OccursCheckFailed {
            var,
            ty: ty.clone(),
        });
    }

    let mut updated_subst = subst.clone();
    updated_subst.insert(var, ty.clone());

    Ok(updated_subst)
}

fn unify(ty1: &Type, ty2: &Type, subst: &Subst) -> Result<Subst, TypeError> {
    let ty1 = apply_subst_type(ty1, subst);
    let ty2 = apply_subst_type(ty2, subst);

    match (&ty1, &ty2) {
        (_, _) if ty1 == ty2 => Ok(subst.clone()),
        (Type::Meta(id), other) | (other, Type::Meta(id)) => unify_var(*id, other, subst),
        (Type::Arrow(a1, b1), Type::Arrow(a2, b2)) => {
            let unified_subst = unify(a1, a2, subst)?;
            unify(b1, b2, &unified_subst)
        }
        (_, _) => Err(TypeError::TypeMismatch {
            expected: ty1.clone(),
            found: ty2.clone(),
            context: None,
        }),
    }
}

pub fn infer(
    term: &Term,
    env: &TypeEnv,
    generator: &mut TypeVarGenerator,
) -> Result<(Subst, Type), TypeError> {
    match term {
        Term::Var(name, _) => {
            if let Some(mapped_scheme) = env.get(name) {
                Ok((HashMap::new(), instantiate(mapped_scheme, generator)))
            } else {
                Err(TypeError::UnboundVar(name.clone()))
            }
        }
        Term::Lambda(param, param_ty, body, _) => {
            let param_ty = if let Some(inner) = param_ty {
                inner.clone()
            } else {
                generator.fresh()
            };

            let mut lambda_env = env.clone();
            lambda_env.insert(
                param.clone(),
                TypeScheme {
                    vars: vec![],
                    ty: param_ty.clone(),
                },
            );

            let (inferred_subst, inferred_body_ty) = infer(body, &lambda_env, generator)?;

            let body_ty = apply_subst_type(&inferred_body_ty, &inferred_subst);

            let subst_param_ty = apply_subst_type(&param_ty, &inferred_subst);

            Ok((
                inferred_subst,
                Type::Arrow(Box::new(subst_param_ty), Box::new(body_ty)),
            ))
        }

        Term::Application(left, right, _) => {
            // infer f
            let (s1, f_ty) = infer(left, env, generator)?;
            // infer a under env updated by s1
            let env1 = apply_subst_env(env, &s1);
            let (s2, a_ty) = infer(right, &env1, generator)?;
            let a_ty = apply_subst_type(&a_ty, &s2);
            // create fresh return type
            let ret_ty = generator.fresh();
            // enforce f_ty ~ a_ty -> ret_ty
            let f_ty = apply_subst_type(&f_ty, &s2);
            let s3 = unify(
                &f_ty,
                &Type::Arrow(Box::new(a_ty.clone()), Box::new(ret_ty.clone())),
                &s2,
            )
            .map_err(|err| err.with_context(right, &a_ty))?;
            // compose substitutions
            let s = compose_subst(&s3, &compose_subst(&s2, &s1));
            // return
            let ret_ty = apply_subst_type(&ret_ty, &s);
            Ok((s, ret_ty))
        }
        Term::Let { name, value, body, .. } => {
            // infer value
            let (s1, v_ty) = infer(value, env, generator)?;
            // update env with s1 and generalize
            let env1 = apply_subst_env(env, &s1);
            let v_ty = apply_subst_type(&v_ty, &s1);
            let scheme = generalize(&v_ty, &env1);
            // extend env with x : scheme
            let mut env2 = env1.clone();
            env2.insert(name.clone(), scheme);
            // infer body in extended env
            let (s2, body_ty) = infer(body, &env2, generator)?;
            // compose substitutions
            let s = compose_subst(&s2, &s1);
            let body_ty = apply_subst_type(&body_ty, &s);
            // return
            Ok((s, body_ty))
        }
    }
}

fn collect_free_vars(term: &Term, bound: &mut HashSet<String>, free: &mut HashSet<String>) {
    match term {
        Term::Var(name, _) => {
            if !bound.contains(name) {
                free.insert(name.clone());
            }
        }
        Term::Lambda(param, _, body, _) => {
            bound.insert(param.clone());
            collect_free_vars(body, bound, free);
            bound.remove(param);
        }
        Term::Application(left, right, _) => {
            collect_free_vars(left, bound, free);
            collect_free_vars(right, bound, free);
        }
        Term::Let { name, value, body, .. } => {
            collect_free_vars(value, bound, free);
            bound.insert(name.clone());
            collect_free_vars(body, bound, free);
            bound.remove(name);
        }
    }
}

pub fn seed_free_vars_term(term: &Term, env: &mut TypeEnv, generator: &mut TypeVarGenerator) {
    let mut bound = HashSet::new();
    let mut free = HashSet::new();
    collect_free_vars(term, &mut bound, &mut free);

    for name in free {
        env.entry(name).or_insert_with(|| TypeScheme {
            vars: vec![],
            ty: generator.fresh(),
        });
    }
}

pub fn seed_free_vars_statement(
    stmt: &Statement,
    env: &mut TypeEnv,
    generator: &mut TypeVarGenerator,
) {
    let mut bound = HashSet::new();
    let mut free = HashSet::new();

    match stmt {
        Statement::Let(name, value, _) => {
            collect_free_vars(value, &mut bound, &mut free);
            free.remove(name);
        }
        Statement::Expr(term, _) => collect_free_vars(term, &mut bound, &mut free),
    }

    for name in free {
        env.entry(name).or_insert_with(|| TypeScheme {
            vars: vec![],
            ty: generator.fresh(),
        });
    }
}

pub fn infer_statement(
    stmt: &Statement,
    env: &mut TypeEnv,
    generator: &mut TypeVarGenerator,
) -> Result<Option<Type>, TypeError> {
    match stmt {
        Statement::Let(name, value, _) => {
            let (subst, ty) = infer(value, env, generator)?;
            let env1 = apply_subst_env(env, &subst);
            let ty = apply_subst_type(&ty, &subst);
            let scheme = generalize(&ty, &env1);
            *env = env1;
            env.insert(name.clone(), scheme);
            Ok(None)
        }
        Statement::Expr(term, _) => {
            let (subst, ty) = infer(term, env, generator)?;
            let ty = apply_subst_type(&ty, &subst);
            Ok(Some(ty))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{lexer::lex, parser::parse_term};

    fn span() -> Span {
        Span { start: 0, end: 0 }
    }

    fn var(name: &str) -> Term {
        Term::Var(name.into(), span())
    }

    fn lam(name: &str, body: Term) -> Term {
        Term::Lambda(name.into(), None, Box::new(body), span())
    }

    fn app(left: Term, right: Term) -> Term {
        Term::Application(Box::new(left), Box::new(right), span())
    }

    fn let_term(name: &str, value: Term, body: Term) -> Term {
        Term::Let {
            name: name.into(),
            value: Box::new(value),
            body: Box::new(body),
            span: span(),
        }
    }

    #[test]
    fn test_free_type_vars_type() {
        assert_eq!(free_type_vars_type(&Type::Var("A".into())), HashSet::new());
        assert_eq!(free_type_vars_type(&Type::Meta(3)), HashSet::from_iter([3]));
        assert_eq!(
            free_type_vars_type(&Type::Arrow(
                Box::new(Type::Meta(1)),
                Box::new(Type::Arrow(
                    Box::new(Type::Var("B".into())),
                    Box::new(Type::Meta(2))
                ))
            )),
            HashSet::from_iter([1, 2])
        );
    }

    #[test]
    fn test_free_type_vars_scheme() {
        let scheme = TypeScheme {
            vars: vec![1],
            ty: Type::Arrow(Box::new(Type::Meta(1)), Box::new(Type::Meta(2))),
        };

        assert_eq!(free_type_vars_scheme(&scheme), HashSet::from_iter([2]));
    }

    #[test]
    fn test_free_type_vars_env() {
        let mut env: TypeEnv = HashMap::new();
        env.insert(
            "id".into(),
            TypeScheme {
                vars: vec![1],
                ty: Type::Arrow(Box::new(Type::Meta(1)), Box::new(Type::Meta(1))),
            },
        );
        env.insert(
            "const".into(),
            TypeScheme {
                vars: vec![],
                ty: Type::Arrow(Box::new(Type::Meta(2)), Box::new(Type::Meta(3))),
            },
        );

        assert_eq!(free_type_vars_env(&env), HashSet::from_iter([2, 3]));
    }

    #[test]
    fn test_generalize_quantifies_free_vars() {
        let env: TypeEnv = HashMap::new();
        let ty = Type::Arrow(Box::new(Type::Meta(1)), Box::new(Type::Meta(2)));

        let scheme = generalize(&ty, &env);

        let vars = scheme.vars.iter().copied().collect::<HashSet<_>>();
        assert_eq!(vars, HashSet::from_iter([1, 2]));
        assert_eq!(scheme.ty, ty);
    }

    #[test]
    fn test_generalize_excludes_env_vars() {
        let mut env: TypeEnv = HashMap::new();
        env.insert(
            "x".into(),
            TypeScheme {
                vars: vec![],
                ty: Type::Meta(1),
            },
        );

        let ty = Type::Arrow(Box::new(Type::Meta(1)), Box::new(Type::Meta(2)));
        let scheme = generalize(&ty, &env);

        assert_eq!(scheme.vars, vec![2]);
    }

    #[test]
    fn test_instantiate_replaces_bound_vars() {
        let scheme = TypeScheme {
            vars: vec![1, 2],
            ty: Type::Arrow(Box::new(Type::Meta(1)), Box::new(Type::Meta(2))),
        };

        let mut generator = TypeVarGenerator::new();
        let instantiated = instantiate(&scheme, &mut generator);

        match instantiated {
            Type::Arrow(left, right) => {
                assert!(matches!(*left, Type::Meta(0)));
                assert!(matches!(*right, Type::Meta(1)));
            }
            _ => panic!("expected arrow type"),
        }
    }

    #[test]
    fn test_apply_subst_type_replaces_metas() {
        let ty = Type::Arrow(Box::new(Type::Meta(1)), Box::new(Type::Meta(2)));
        let subst = HashMap::from_iter([
            (1, Type::Var("A".into())),
            (
                2,
                Type::Arrow(
                    Box::new(Type::Var("B".into())),
                    Box::new(Type::Var("C".into())),
                ),
            ),
        ]);

        let applied = apply_subst_type(&ty, &subst);

        assert_eq!(
            applied,
            Type::Arrow(
                Box::new(Type::Var("A".into())),
                Box::new(Type::Arrow(
                    Box::new(Type::Var("B".into())),
                    Box::new(Type::Var("C".into()))
                ))
            )
        );
    }

    #[test]
    fn test_apply_subst_scheme_ignores_bound_vars() {
        let scheme = TypeScheme {
            vars: vec![1],
            ty: Type::Arrow(Box::new(Type::Meta(1)), Box::new(Type::Meta(2))),
        };
        let subst = HashMap::from_iter([(1, Type::Var("A".into())), (2, Type::Var("B".into()))]);

        let applied = apply_subst_scheme(&scheme, &subst);

        assert_eq!(
            applied.ty,
            Type::Arrow(Box::new(Type::Meta(1)), Box::new(Type::Var("B".into())))
        );
        assert_eq!(applied.vars, vec![1]);
    }

    #[test]
    fn test_apply_subst_env_updates_all_schemes() {
        let mut env: TypeEnv = HashMap::new();
        env.insert(
            "x".into(),
            TypeScheme {
                vars: vec![],
                ty: Type::Meta(1),
            },
        );
        env.insert(
            "id".into(),
            TypeScheme {
                vars: vec![2],
                ty: Type::Arrow(Box::new(Type::Meta(2)), Box::new(Type::Meta(3))),
            },
        );

        let subst = HashMap::from_iter([(1, Type::Var("A".into())), (3, Type::Var("B".into()))]);

        let applied = apply_subst_env(&env, &subst);

        assert_eq!(applied["x"].ty, Type::Var("A".into()));
        assert_eq!(
            applied["id"].ty,
            Type::Arrow(Box::new(Type::Meta(2)), Box::new(Type::Var("B".into())))
        );
    }

    #[test]
    fn test_compose_subst_merges_and_applies() {
        let s1 = HashMap::from_iter([(1, Type::Var("A".into())), (2, Type::Meta(3))]);
        let s2 = HashMap::from_iter([(2, Type::Var("B".into())), (4, Type::Meta(1))]);

        let composed = compose_subst(&s1, &s2);

        assert_eq!(composed.get(&2), Some(&Type::Var("B".into())));
        assert_eq!(composed.get(&4), Some(&Type::Var("A".into())));
        assert_eq!(composed.get(&1), Some(&Type::Var("A".into())));
    }

    #[test]
    fn test_occurs_detects_nested_meta() {
        let ty = Type::Arrow(Box::new(Type::Meta(1)), Box::new(Type::Var("A".into())));
        assert!(occurs(1, &ty));
        assert!(!occurs(2, &ty));
    }

    #[test]
    fn test_unify_var_binds_meta() {
        let subst = HashMap::new();
        let updated = unify_var(1, &Type::Var("A".into()), &subst).unwrap();
        assert_eq!(updated.get(&1), Some(&Type::Var("A".into())));
    }

    #[test]
    fn test_unify_var_occurs_check() {
        let subst = HashMap::new();
        let ty = Type::Arrow(Box::new(Type::Meta(1)), Box::new(Type::Var("A".into())));
        let err = unify_var(1, &ty, &subst).unwrap_err();
        assert!(matches!(err, TypeError::OccursCheckFailed { .. }));
    }

    #[test]
    fn test_unify_arrows() {
        let subst = HashMap::new();
        let ty1 = Type::Arrow(Box::new(Type::Meta(1)), Box::new(Type::Var("A".into())));
        let ty2 = Type::Arrow(Box::new(Type::Var("B".into())), Box::new(Type::Meta(2)));
        let unified = unify(&ty1, &ty2, &subst).unwrap();
        assert_eq!(unified.get(&1), Some(&Type::Var("B".into())));
        assert_eq!(unified.get(&2), Some(&Type::Var("A".into())));
    }

    #[test]
    fn test_unify_mismatch() {
        let subst = HashMap::new();
        let err = unify(&Type::Var("A".into()), &Type::Var("B".into()), &subst).unwrap_err();
        assert!(matches!(err, TypeError::TypeMismatch { .. }));
    }

    #[test]
    fn test_infer_identity_lambda() {
        let term = lam("x", var("x"));
        let env: TypeEnv = HashMap::new();
        let mut generator = TypeVarGenerator::new();

        let (_subst, ty) = infer(&term, &env, &mut generator).unwrap();

        match ty {
            Type::Arrow(left, right) => {
                assert_eq!(*left, *right);
                assert!(matches!(*left, Type::Meta(_)));
            }
            _ => panic!("expected arrow type"),
        }
    }

    #[test]
    fn test_infer_composition_shape() {
        let term = lam("f", lam("x", app(var("f"), var("x"))));
        let env: TypeEnv = HashMap::new();
        let mut generator = TypeVarGenerator::new();

        let (_subst, ty) = infer(&term, &env, &mut generator).unwrap();

        match ty {
            Type::Arrow(f_ty, body_ty) => match (*f_ty, *body_ty) {
                (Type::Arrow(a1, b1), Type::Arrow(a2, b2)) => {
                    assert_eq!(*a1, *a2);
                    assert_eq!(*b1, *b2);
                }
                _ => panic!("expected arrow types"),
            },
            _ => panic!("expected arrow type"),
        }
    }

    #[test]
    fn test_infer_let_polymorphism() {
        let term = let_term("id", lam("x", var("x")), app(var("id"), var("id")));
        let env: TypeEnv = HashMap::new();
        let mut generator = TypeVarGenerator::new();

        let (_subst, ty) = infer(&term, &env, &mut generator).unwrap();

        match ty {
            Type::Arrow(left, right) => {
                assert_eq!(*left, *right);
            }
            _ => panic!("expected arrow type"),
        }
    }

    #[test]
    fn test_infer_occurs_check_error() {
        let term = lam("x", app(var("x"), var("x")));
        let env: TypeEnv = HashMap::new();
        let mut generator = TypeVarGenerator::new();

        let err = infer(&term, &env, &mut generator).unwrap_err();
        assert!(matches!(err, TypeError::OccursCheckFailed { .. }));
    }

    #[test]
    fn test_type_mismatch_span_points_to_argument() {
        let term = parse_term(lex("(\\x: A. x) (\\y: B. y)").unwrap()).unwrap();
        let arg_span = match &term {
            Term::Application(_, right, _) => right.span(),
            _ => panic!("expected application"),
        };
        let env: TypeEnv = HashMap::new();
        let mut generator = TypeVarGenerator::new();

        let err = infer(&term, &env, &mut generator).unwrap_err();
        match err {
            TypeError::TypeMismatch {
                context: Some(ctx),
                ..
            } => {
                assert_eq!(ctx.span.start, arg_span.start);
                assert_eq!(ctx.span.end, arg_span.end);
            }
            _ => panic!("expected type mismatch with context"),
        }
    }
}
