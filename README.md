# λ lambda — Lambda Calculus Interpreter in Rust

`lambda` is a lightweight, REPL-driven lambda calculus interpreter built in Rust for exploring parsing, evaluation, and Hindley-Milner style type inference.
It combines a small interactive language, predictable pretty-printing, and span-aware diagnostics to make experimentation fast and readable.

---

## 🧠 Overview

`lambda` is a recreational interpreter for untyped and simply annotated lambda calculus expressions with a type inference layer designed for learning and experimentation.

- **Interactive evaluation** happens through a terminal REPL with immediate feedback.
- **Parsing and pretty-printing** keep lambda terms readable and stable across inputs.
- **Evaluation** supports both call-by-value and call-by-name semantics at the engine level.
- **Type inference** uses a Hindley-Milner style system with a primitive `Bool` type.
- **Diagnostics** report lexing, parsing, and typing failures with source spans.

---

## ✨ Features

- **REPL-first workflow** for evaluating expressions and defining global `let` bindings
- **Lambda calculus parser** with predictable pretty-printed output
- **Call-by-value and call-by-name** evaluation modes in the interpreter core
- **Hindley-Milner style inference** with support for explicit annotations
- **Builtin boolean environment** with `true`, `false`, `if`, `and`, `or`, and `not`
- **Span-aware error messages** that point to the exact source location of failures

---

## 🧩 Type System

`lambda` infers types without requiring annotations on every term.
When an annotation is present, it is checked against the inferred structure and reported with precise spans on mismatch.

> **Note:**
> The project is named `lambda`, while the current Cargo package and crate name in `Cargo.toml` is still `lc`.

This project treats booleans as a primitive type at the type level (`Bool`) while still evaluating Church-encoded boolean terms at runtime. That lets expressions such as `if true (not true) false` typecheck as `Bool` without needing impredicative polymorphism.

---

## 🚀 Quick Start

**Requirements:**

- Rust 1.85+ with Cargo

**Run the REPL:**

```bash
cargo run
```

**Run the test suite:**

```bash
cargo test
```

---

## 💬 REPL Examples

### Identity

```text
λ> \x.x
\x.x : t0 -> t0
```

### Let binding

```text
λ> let id = \x.x in id y
y : t3
```

### Type annotations

```text
λ> \x: Bool. x
\x.x : Bool -> Bool
```

```text
λ> (\x: Bool -> Bool. x) (\y: Bool. y)
\y.y : Bool -> Bool

λ> (\x: (A -> A) -> (A -> A).x)(\y: A -> A.y)
\y.y : (A -> A) -> A -> A
```

### Builtin booleans

```text
λ> if true (not true) false
\t.\f.f : Bool
```

### Syntax error reporting

```text
λ> \x.
error: expected term
 --> line 1, col 4
  |
1 | \x.
  |    ^
```

---

## 🏗️ Project Layout

- `src/repl.rs` — interactive terminal loop and user-facing error formatting
- `src/parser.rs` / `src/lexer.rs` — tokenization and parsing for lambda terms and statements
- `src/eval.rs` — normalization, substitution, and evaluation strategies
- `src/typing.rs` — Hindley-Milner style inference and type errors
- `src/interpreter.rs` — top-level environment, builtins, and statement execution

---

## ⚠️ Current Limitations & Future Work

While `lambda` is useful as an educational interpreter, it is intentionally small and leaves room for deeper language features:

| Area                 | Limitation                                                                     | Potential Improvement                                                    |
| -------------------- | ------------------------------------------------------------------------------ | ------------------------------------------------------------------------ |
| **REPL UX**          | No command system for toggling evaluation mode or inspecting environment state | Add commands such as `:mode`, `:env`, and `:type`                        |
| **Types**            | Only a primitive `Bool` is built in                                            | Add richer primitives such as integers, tuples, and algebraic data types |
| **Evaluation**       | Reduction tracing is printed directly during normalization                     | Make tracing configurable or expose a dedicated debug mode               |
| **Persistence**      | REPL state is session-local only                                               | Support loading source files or a standard prelude                       |
| **Language surface** | Focused on core lambda calculus plus `let`                                     | Add syntax sugar for multi-argument functions or local declarations      |
