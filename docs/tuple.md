# RCA Tuple (design)

Status: DESIGN ONLY. Nothing here is implemented yet.

## 1. Scope of the first version
- Fixed-size, immutable, mixed-type values: `let p = (1, "a")`.
- At least 2 elements. `(x)` stays plain grouping. `()` and `(x,)` are later.
- Access by literal index only: `p[0]`, `p[1]`. The checker knows each element type.
- `len(p)` returns the element count.
- `==` and `!=` between tuples of the same type.
- Printing shows `(1, "a")`.
- No tuple type annotations yet (`let p: (Int, String)` is a later change,
  because `parse_type` only reads a single name today).
- No destructuring (`let (a, b) = p`) yet.

## 2. Representation
- Type: `Type::Tuple(Vec<Type>)`, displayed as `(Int, String)`.
- Value: `Value::Tuple(Vec<Value>)`.
- Value semantics (copy on assignment), like every RCA value.
- Immutable: `p[0] = x` is a compile-time error.

## 3. Checker rules
- `p[i]`: `i` must be an Int literal, in range, else a compile-time error
  naming the tuple length. A non-literal index is an error.
- Element types are never mixed up: `p[0]` has the exact type of element 0.

## 4. Parser
- In the `LParen` case: after the first expression, if a `,` follows, parse
  the remaining expressions until `)` and build `ExprKind::Tuple(items)`.
- Otherwise it is grouping, as today.

## 5. Milestones (one at a time)
T1: AST `ExprKind::Tuple` + parser + tests (checker/lowering placeholder).
T2: checker: `Type::Tuple`, literal indexing, `len`, `==`.
T3: runtime: `Value::Tuple`, IR `MakeTuple(n)` and `TupleGet(i)`, VM and tree identical.
T4: examples, docs, tag.

## 6. Rules for future changes
- Interpreter and VM must give identical results.
- Every feature needs tests with known expected values.
