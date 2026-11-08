# RCA Tuple

Status: IMPLEMENTED (first version). Milestones T1-T3 are done.
VM and tree interpreter give identical results (tests in vm/mod.rs).

## 1. What works
- Fixed-size, immutable, mixed-type values with 2 or more elements:
  `let p = (1, "a", 2.5)`. A trailing comma is allowed.
- `(x)` is plain grouping, not a tuple.
- Access by literal index: `p[0]`, `p[1]`. Each element keeps its exact type.
- `len(p)` returns the element count.
- `==` and `!=` between tuples of the same type.
- Printing shows `(1, "a", 2.5)`.

## 2. Checker rules
- The index must be an Int literal and in range, otherwise a compile-time error
  (`tuple index N out of range for tuple of length L`,
  `tuple index must be an Int literal`).
- `p[i] = x` is an error: `tuples are immutable, cannot assign to an element`.
- Comparing tuples of different types is an error (`cannot compare (Int, Int) with (Int, String)`).
- A tuple with fewer than 2 elements, such as `(1,)`, is a parse error.

## 3. Implementation notes
- Type: `Type::Tuple(Vec<Type>)`. Value: `Value::Tuple(Vec<Value>)`.
- AST: `ExprKind::Tuple(Vec<Expr>)`, parsed in the `LParen` case when a comma follows the first expression.
- IR: `MakeTuple(n)`. Indexing reuses the existing `Index` instruction.
- Equality reuses `apply_binary`; no new instruction was needed.

## 4. Not implemented yet
- Tuple types in struct fields and nested tuple types are untested.
- Destructuring (`let (a, b) = p`).
- Empty and one-element tuples.
- Non-literal indexing.


## 5. Rules for future changes
- Interpreter and VM must give identical results.
- Every feature needs tests with known expected values.

## 6. Type annotations (v1.3)
- `let p: (Int, String) = (1, "a")`
- `fn swap(a: Int, b: String) -> (String, Int)` works for parameters and return types.
- A tuple type needs at least 2 elements: `(Int)` is a parse error.
