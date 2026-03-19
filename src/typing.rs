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
    /// Formats type errors for display.
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
    /// Returns a human-friendly error message.
    pub fn message(&self) -> String {
        self.to_string()
    }

    /// Adds term and type context to a mismatch error.
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
    Bool,
}

impl Display for Type {
    /// Formats types for display.
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
            Type::Bool => write!(f, "Bool"),
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
    /// Creates a new type variable generator.
    pub fn new() -> TypeVarGenerator {
        TypeVarGenerator { next: 0u32 }
    }

    /// Generates a fresh meta type variable.
    fn fresh(&mut self) -> Type {
        let fresh_type = Type::Meta(self.next);
        self.next += 1;
        fresh_type
    }
}

impl Default for TypeVarGenerator {
    /// Creates a default type variable generator.
    fn default() -> Self {
        Self::new()
    }
}

/// Collects free type variables in a type.
fn free_type_vars_type(ty: &Type) -> HashSet<TypeVar> {
    match ty {
        Type::Var(_) => HashSet::new(),
        Type::Meta(id) => HashSet::from_iter([*id]),
        Type::Arrow(left, right) => {
            let mut free_vars = free_type_vars_type(left);
            free_vars.extend(free_type_vars_type(right));
            free_vars
        }
        Type::Bool => HashSet::new(),
    }
}

/// Collects free type variables in a type scheme.
fn free_type_vars_scheme(scheme: &TypeScheme) -> HashSet<TypeVar> {
    let mut free_vars = free_type_vars_type(&scheme.ty);

    for var in scheme.vars.iter() {
        free_vars.remove(var);
    }

    free_vars
}

/// Collects free type variables in a type environment.
fn free_type_vars_env(env: &TypeEnv) -> HashSet<TypeVar> {
    let mut free_vars = HashSet::new();

    for scheme in env.values() {
        free_vars.extend(free_type_vars_scheme(scheme));
    }

    free_vars
}

/// Generalizes a type with respect to an environment.
fn generalize(ty: &Type, env: &TypeEnv) -> TypeScheme {
    let free_type_vars = free_type_vars_type(ty);
    let free_env_vars = free_type_vars_env(env);

    TypeScheme {
        vars: free_type_vars.difference(&free_env_vars).copied().collect(),
        ty: ty.clone(),
    }
}

/// Instantiates a type scheme with fresh meta variables.
fn instantiate(scheme: &TypeScheme, generator: &mut TypeVarGenerator) -> Type {
    let mapping = HashMap::from_iter(scheme.vars.iter().map(|var| (*var, generator.fresh())));
    replace_meta(&scheme.ty, &mapping)
}

/// Replaces meta variables using a mapping.
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
        Type::Bool => Type::Bool,
    }
}

/// Applies a substitution to a type.
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
        Type::Bool => Type::Bool,
    }
}

/// Applies a substitution to a type scheme.
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

/// Applies a substitution to every scheme in an environment.
fn apply_subst_env(env: &TypeEnv, subst: &Subst) -> TypeEnv {
    env.iter()
        .map(|(key, scheme)| (key.clone(), apply_subst_scheme(scheme, subst)))
        .collect()
}

/// Composes two substitutions.
fn compose_subst(subst1: &Subst, subst2: &Subst) -> Subst {
    let mut combined_subst: HashMap<_, _> = subst2
        .iter()
        .map(|(key, mapping)| (*key, apply_subst_type(mapping, subst1)))
        .collect();

    for (key, mapping) in subst1.iter() {
        combined_subst
            .entry(*key)
            .or_insert_with(|| mapping.clone());
    }

    combined_subst
}

/// Checks whether a type variable occurs within a type.
fn occurs(type_var: TypeVar, ty: &Type) -> bool {
    match ty {
        Type::Var(_) => false,
        Type::Meta(id) => type_var == *id,
        Type::Arrow(left, right) => occurs(type_var, left) || occurs(type_var, right),
        Type::Bool => false,
    }
}

/// Unifies a meta variable with a type.
fn unify_var(type_var: TypeVar, ty: &Type, subst: &Subst) -> Result<Subst, TypeError> {
    let ty = apply_subst_type(ty, subst);

    if let Type::Meta(id) = ty
        && id == type_var
    {
        return Ok(subst.clone());
    }

    if occurs(type_var, &ty) {
        return Err(TypeError::OccursCheckFailed {
            var: type_var,
            ty: ty.clone(),
        });
    }

    let mut updated_subst = subst.clone();
    updated_subst.insert(type_var, ty.clone());

    Ok(updated_subst)
}

/// Unifies two types with the given substitution.
fn unify(left_ty: &Type, right_ty: &Type, subst: &Subst) -> Result<Subst, TypeError> {
    let left_ty = apply_subst_type(left_ty, subst);
    let right_ty = apply_subst_type(right_ty, subst);

    match (&left_ty, &right_ty) {
        (_, _) if left_ty == right_ty => Ok(subst.clone()),
        (Type::Meta(id), other) | (other, Type::Meta(id)) => unify_var(*id, other, subst),
        (Type::Bool, Type::Bool) => Ok(subst.clone()),
        (Type::Arrow(left_arg, left_ret), Type::Arrow(right_arg, right_ret)) => {
            let subst1 = unify(left_arg, right_arg, subst)?;
            unify(left_ret, right_ret, &subst1)
        }
        (_, _) => Err(TypeError::TypeMismatch {
            expected: left_ty.clone(),
            found: right_ty.clone(),
            context: None,
        }),
    }
}

/// Infers the type of a term.
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

            let mut env1 = env.clone();
            env1.insert(
                param.clone(),
                TypeScheme {
                    vars: vec![],
                    ty: param_ty.clone(),
                },
            );

            let (subst1, inferred_body_ty) = infer(body, &env1, generator)?;

            let body_ty = apply_subst_type(&inferred_body_ty, &subst1);

            let param_ty = apply_subst_type(&param_ty, &subst1);

            Ok((
                subst1,
                Type::Arrow(Box::new(param_ty), Box::new(body_ty)),
            ))
        }

        Term::Application(left, right, _) => {
            let (subst1, func_ty) = infer(left, env, generator)?;
            let env1 = apply_subst_env(env, &subst1);
            let (subst2, arg_ty) = infer(right, &env1, generator)?;
            let arg_ty = apply_subst_type(&arg_ty, &subst2);
            let ret_ty = generator.fresh();
            let func_ty = apply_subst_type(&func_ty, &subst2);
            let subst3 = unify(
                &func_ty,
                &Type::Arrow(Box::new(arg_ty.clone()), Box::new(ret_ty.clone())),
                &subst2,
            )
            .map_err(|err| err.with_context(right, &arg_ty))?;
            let subst = compose_subst(&subst3, &compose_subst(&subst2, &subst1));
            let ret_ty = apply_subst_type(&ret_ty, &subst);
            Ok((subst, ret_ty))
        }
        Term::Let { name, value, body, .. } => {
            let (subst1, value_ty) = infer(value, env, generator)?;
            let env1 = apply_subst_env(env, &subst1);
            let value_ty = apply_subst_type(&value_ty, &subst1);
            let scheme = generalize(&value_ty, &env1);
            let mut env2 = env1.clone();
            env2.insert(name.clone(), scheme);
            let (subst2, body_ty) = infer(body, &env2, generator)?;
            let subst = compose_subst(&subst2, &subst1);
            let body_ty = apply_subst_type(&body_ty, &subst);
            Ok((subst, body_ty))
        }
    }
}

/// Collects free term variables for type inference.
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

/// Seeds the type environment with free variables from a term.
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

/// Seeds the type environment with free variables from a statement.
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

/// Infers the type of a statement.
pub fn infer_statement(
    stmt: &Statement,
    env: &mut TypeEnv,
    generator: &mut TypeVarGenerator,
) -> Result<Option<Type>, TypeError> {
    match stmt {
        Statement::Let(name, value, _) => {
            let (subst1, value_ty) = infer(value, env, generator)?;
            let env1 = apply_subst_env(env, &subst1);
            let value_ty = apply_subst_type(&value_ty, &subst1);
            let scheme = generalize(&value_ty, &env1);
            *env = env1;
            env.insert(name.clone(), scheme);
            Ok(None)
        }
        Statement::Expr(term, _) => {
            let (subst, expr_ty) = infer(term, env, generator)?;
            let expr_ty = apply_subst_type(&expr_ty, &subst);
            Ok(Some(expr_ty))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{lexer::lex, parser::parse_term};

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

    /// Builds a let term with a dummy span.
    fn let_term(name: &str, value: Term, body: Term) -> Term {
        Term::Let {
            name: name.into(),
            value: Box::new(value),
            body: Box::new(body),
            span: span(),
        }
    }

    #[test]
    /// Ensures free type variables are collected from types.
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
    /// Ensures free type variables are collected from schemes.
    fn test_free_type_vars_scheme() {
        let scheme = TypeScheme {
            vars: vec![1],
            ty: Type::Arrow(Box::new(Type::Meta(1)), Box::new(Type::Meta(2))),
        };

        assert_eq!(free_type_vars_scheme(&scheme), HashSet::from_iter([2]));
    }

    #[test]
    /// Ensures free type variables are collected from environments.
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
    /// Ensures generalize quantifies free variables.
    fn test_generalize_quantifies_free_vars() {
        let env: TypeEnv = HashMap::new();
        let ty = Type::Arrow(Box::new(Type::Meta(1)), Box::new(Type::Meta(2)));

        let scheme = generalize(&ty, &env);

        let vars = scheme.vars.iter().copied().collect::<HashSet<_>>();
        assert_eq!(vars, HashSet::from_iter([1, 2]));
        assert_eq!(scheme.ty, ty);
    }

    #[test]
    /// Ensures generalize excludes environment variables.
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
    /// Ensures instantiate replaces bound variables.
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
    /// Ensures substitutions replace meta variables in types.
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
    /// Ensures substitutions ignore scheme bound vars.
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
    /// Ensures substitutions apply across the environment.
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
    /// Ensures composition merges and applies substitutions.
    fn test_compose_subst_merges_and_applies() {
        let s1 = HashMap::from_iter([(1, Type::Var("A".into())), (2, Type::Meta(3))]);
        let s2 = HashMap::from_iter([(2, Type::Var("B".into())), (4, Type::Meta(1))]);

        let composed = compose_subst(&s1, &s2);

        assert_eq!(composed.get(&2), Some(&Type::Var("B".into())));
        assert_eq!(composed.get(&4), Some(&Type::Var("A".into())));
        assert_eq!(composed.get(&1), Some(&Type::Var("A".into())));
    }

    #[test]
    /// Ensures occurs check detects nested metas.
    fn test_occurs_detects_nested_meta() {
        let ty = Type::Arrow(Box::new(Type::Meta(1)), Box::new(Type::Var("A".into())));
        assert!(occurs(1, &ty));
        assert!(!occurs(2, &ty));
    }

    #[test]
    /// Ensures unify_var binds a meta variable.
    fn test_unify_var_binds_meta() {
        let subst = HashMap::new();
        let updated = unify_var(1, &Type::Var("A".into()), &subst).unwrap();
        assert_eq!(updated.get(&1), Some(&Type::Var("A".into())));
    }

    #[test]
    /// Ensures unify_var performs the occurs check.
    fn test_unify_var_occurs_check() {
        let subst = HashMap::new();
        let ty = Type::Arrow(Box::new(Type::Meta(1)), Box::new(Type::Var("A".into())));
        let err = unify_var(1, &ty, &subst).unwrap_err();
        assert!(matches!(err, TypeError::OccursCheckFailed { .. }));
    }

    #[test]
    /// Ensures unification works for arrow types.
    fn test_unify_arrows() {
        let subst = HashMap::new();
        let ty1 = Type::Arrow(Box::new(Type::Meta(1)), Box::new(Type::Var("A".into())));
        let ty2 = Type::Arrow(Box::new(Type::Var("B".into())), Box::new(Type::Meta(2)));
        let unified = unify(&ty1, &ty2, &subst).unwrap();
        assert_eq!(unified.get(&1), Some(&Type::Var("B".into())));
        assert_eq!(unified.get(&2), Some(&Type::Var("A".into())));
    }

    #[test]
    /// Ensures unification fails on mismatched types.
    fn test_unify_mismatch() {
        let subst = HashMap::new();
        let err = unify(&Type::Var("A".into()), &Type::Var("B".into()), &subst).unwrap_err();
        assert!(matches!(err, TypeError::TypeMismatch { .. }));
    }

    #[test]
    /// Ensures inference works for identity lambdas.
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
    /// Ensures inference preserves composition structure.
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
    /// Ensures let-polymorphism is inferred.
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
    /// Ensures inference reports occurs check errors.
    fn test_infer_occurs_check_error() {
        let term = lam("x", app(var("x"), var("x")));
        let env: TypeEnv = HashMap::new();
        let mut generator = TypeVarGenerator::new();

        let err = infer(&term, &env, &mut generator).unwrap_err();
        assert!(matches!(err, TypeError::OccursCheckFailed { .. }));
    }

    #[test]
    /// Ensures type mismatch spans point to the argument.
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
