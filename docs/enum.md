# RCA enum and match

Status: IMPLEMENTED (first version). Milestones E1-E7 are done.
VM and tree interpreter give identical results (tests in vm/mod.rs).

## 1. What works
- Simple enums (variants carry no data), `pub enum`.
- Variants are always written qualified: `Color.Red`.
- Enum names are types: `let c: Color = Color.Green`, `fn f(c: Color)`.
- `==` and `!=` between values of the same enum type.
- Printing shows `Color.Green`.
- `match` statement with patterns: enum variant, Int / String / Bool literal, `_`.

## 2. Syntax
    enum Color:
        Red
        Green
        Blue

    match c:
        Color.Red:
            print("red")
        Color.Green:
            print("green")
        Color.Blue:
            print("blue")

    match n:
        0:
            print("zero")
        _:
            print("other")

## 3. Checker rules
- Variant names inside one enum must be unique.
- An enum name must not clash with a struct, function or other enum.
- Match subject must be Int, String, Bool or an enum. Float is an error.
- Every pattern must have the subject's type.
- Duplicate patterns are an error.
- Enum match without `_` must list every variant (missing ones are named).
- Bool match must cover true and false, or use `_`.
- Int and String match need a `_` arm.
- Arms after `_` are an error (unreachable).
- Enums can only be declared at the top level.

## 4. Implementation notes
- Type: `Type::Enum(name)`. Value: `Value::Enum { name, variant }`.
- IR: `PushEnum(name, variant)`.
- `Color.Red` parses as field access; the checker, compiler and interpreter
  treat `Ident.Name` as a variant when `Ident` is an enum and not a variable.
- `match` is desugared (`ast::desugar_match`) into an if/else chain with a
  hidden variable, so the subject is evaluated once and VM and interpreter
  share one definition. No new jump instructions were needed.

## 5. Not implemented yet
- Variants with payload data.
- `match` as an expression.
- Guards and nested patterns.
- Importing enums across modules (`from utils import Color`) is untested.
- "Must return on all paths" does not treat `match` as returning.

## 6. Rules for future changes
- Interpreter and VM must give identical results.
- Every feature needs tests with known expected values.
