# RCA enum and match (design)

Status: DESIGN ONLY. Nothing here is implemented yet.
Tokens `enum` and `match` already exist in the lexer. The parser, AST,
checker, IR, VM and interpreter do not support them yet.

## 1. Scope of the first version
- Simple enums only: variants have NO payload data.
- `match` is a statement (not an expression) in this version.
- Patterns: enum variant, `_` (wildcard), Int literal, String literal, Bool literal.
- No guards, no nested patterns, no payload variants, no generics.
  These come in later milestones.

## 2. Syntax
    enum Color:
        Red
        Green
        Blue

    let c = Color.Green

    match c:
        Color.Red:
            print("red")
        Color.Green:
            print("green")
        _:
            print("other")

    match n:
        0:
            print("zero")
        1:
            print("one")
        _:
            print("many")

- Indentation defines blocks, as everywhere in RCA.
- Variants are written `EnumName.Variant` (qualified), never bare.
- Enum name is a new type: `let c: Color = Color.Red`.

## 3. Representation
- Runtime: `Value::Enum { name: String, variant: String }`.
- Type: `Type::Enum(String)`, like `Type::Struct(String)`.
- Value semantics (copy on assignment), like all RCA values.
- Equality `==` and `!=` work between values of the same enum type.
- Printing shows `Color.Green`.

## 4. Checker rules
- Variant names inside one enum must be unique.
- Enum names must not clash with struct, function or other enum names.
- `match` subject must be Int, String, Bool or an enum type.
- Each pattern must have the same type as the subject.
- Duplicate patterns are an error.
- Exhaustiveness: matching on an enum without `_` must list every variant,
  otherwise a compile-time error naming the missing variants.
- Int and String matches need a `_` arm (they cannot be exhaustive), else error.

## 5. Implementation milestones (one at a time)
E1: AST nodes + parser for `enum` declaration, with tests.
E2: checker: enum type, `Color.Red` expression, `==`.
E3: runtime value + print + equality (VM and interpreter identical).
E4: AST + parser for `match` statement, with tests.
E5: checker for match: pattern types, duplicates, exhaustiveness.
E6: IR lowering, VM and interpreter for match.
E7: examples, docs update, tag.

## 6. Open questions (decide before E1)
- Qualified `Color.Red` parses today as Field access on an Ident `Color`.
  Plan: the checker resolves `Ident.Name` as an enum variant when `Ident`
  is an enum name (no new expression node needed).
- Should `match` later become an expression? Deferred.

## 7. Rules for future changes
- Interpreter and VM must give identical results.
- Every feature needs tests with known expected values.
