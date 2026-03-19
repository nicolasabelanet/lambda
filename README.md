# Lambda

Lambda is a recreational lambda calculus interpreter for educational purposes.

## Features

- Lambda calculus parser and pretty-printer with predictable output
- REPL for interactive evaluation and quick feedback
- Call-by-value and call-by-name evaluation modes
- Hindley-Milner style type inference with a primitive `Bool`
- Span-aware errors for lexing, parsing, and typing
- Builtin terms: `true`, `false`, `if`, `and`, `or`, `not` (Church-encoded for eval)

## Type Checking

Lambda uses Hindley-Milner style inference to assign types without annotations.
Type errors include spans so the REPL can point to the exact subterm that failed.

This project treats booleans as a primitive type at the type level (`Bool`), while
still evaluating Church-encoded boolean terms at runtime. That means expressions
like `if true (not true) false` typecheck as `Bool` without requiring impredicative
polymorphism.

## Usage

Start the REPL:

```bash
cargo run
```

Run tests:

```bash
cargo test
```

## Examples

Identity:

```text
λ> \x.x
\x.x : t0 -> t0
```

Let binding:

```text
λ> let id = \x.x in id y
y : t3
```

Type annotations:

```text
λ> \x: Bool. x
\x.x : Bool -> Bool
```

```text
λ> (\x: Bool -> Bool. x) (\y: Bool. y)
\y.y : Bool -> Bool
```

Booleans:

```text
λ> if true (not true) false
\t.\f.f : Bool
```

Syntax error:

```text
λ> \x.
error: expected term
 --> line 1, col 4
  |
1 | \x.
  |    ^
```
