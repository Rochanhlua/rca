# RCA CLI

Two binaries ship from this crate:

- `rcac` — the compiler and tool driver. All non-run subcommands live here.
- `rca` — a thin front end. `rca <file.rca>` delegates to `rcac run <file.rca>`.

`rcac` with no arguments or with an unknown command prints usage and exits.

## 1. Command summary

    rcac lex        <file.rca>   show tokens
    rcac parse      <file.rca>   show the syntax tree
    rcac check      <file.rca>   type check only
    rcac lint       <file.rca>   report warnings (unused vars, unreachable code)
    rcac ir         <file.rca>   show bytecode
    rcac ir-opt     <file.rca>   show optimized bytecode
    rcac run        <file.rca>   run on the bytecode VM
    rcac run-tree   <file.rca>   run on the tree-walking interpreter
    rcac compile    <file.rca>   generate native object (<file>.rca.o)
    rcac build      <file.rca>   compile and link a native executable
    rcac pkg        <action>     package manager (section 3)

## 2. Notes per command

- `lex` / `parse` / `check` are read-only; useful for debugging.
- `ir` prints the unoptimized bytecode; `ir-opt` prints it after optimization.
- `run` uses the bytecode VM. It supports `import` (user modules and the standard
  library modules listed in `docs/modules.md`).
- `run-tree` uses the tree-walking interpreter. It does **not** support `import`
  yet (see `docs/modules.md`, section 7).
- `compile` writes a `.o` file next to the source (`<file>.rca.o`). Uses Cranelift.
- `build` also links. It invokes `clang` and requires clang on PATH.

## 3. rcac pkg

    rcac pkg init                create rca.json
    rcac pkg add <name> <src>    insert or update a dependency
    rcac pkg install             fetch deps to $HOME/.rca/packages/

See `docs/packages.md` for the manifest format, source types, and import
resolution order.

## 4. Exit codes

| Code | Meaning |
|------|---------|
| 0    | success |
| 1    | compile error, runtime error, or a `pkg` action that failed |

Error messages go to stderr. Successful `print(...)` output goes to stdout.

## 5. Examples

    rcac lex hello.rca
    rcac check hello.rca
    rcac run hello.rca
    rcac build hello.rca && ./hello

    mkdir proj && cd proj
    rcac pkg init
    rcac pkg add mylib ./mylib.rca
    rcac pkg install

## 6. Not in this milestone
- `rcac test` (test runner)
- `rcac fmt` (formatter)
- `rcac lsp` (language server)
- Watch mode
- Shell completion

## 7. See also
- `docs/SYNTAX.md` — language syntax
- `docs/modules.md` — imports and modules
- `docs/packages.md` — package manager
- `docs/memory-model.md`, `docs/tensor.md`, `docs/nn.md` — runtime topics
