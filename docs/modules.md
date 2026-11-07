# RCA Modules (v0.5 design)

Status: DESIGN ONLY. Nothing here is implemented yet.

## 1. Basics
- Every .rca file is a module. The module name is the file name without ".rca".
- `import utils` loads utils.rca from the same directory as the importing file.
- Module members are used with a dot: `utils.add(1, 2)`.

## 2. Import forms
    import utils
    import utils as u
    from utils import add
    from utils import add as plus, Point

## 3. Visibility
- Declarations are private by default.
- `pub fn`, `pub struct`, `pub let`, `pub const` can be imported.
- Importing a private name is a compile-time error.

## 4. Rules
- Each module is loaded and checked once.
- Circular imports are a compile-time error.
- A missing module file is a compile-time error with source location.
- Top-level code of an imported module does not run twice.

## 5. Not in this milestone
- Subdirectories / packages (`import a.b`)
- Standard library modules (math, tensor, ...)
- Package manager

## 6. Implementation order
1. Parser: import / from / as / pub
2. Semantic: module loading, name resolution, visibility
3. IR / VM: run imported modules
4. Tests and examples after each step

## 7. Implementation status (v0.5)

Implemented:
- Parsing: import, import as, from-import, from-import as, pub fn, pub struct
- Module loader: finds NAME.rca next to the importing file, detects missing modules and circular imports, loads each module once
- Checker: from-import of pub functions and structs, private/missing name errors, type checking of imported calls
- rca check and rca run work across files (from-import form)
- Same names in different modules do not clash (functions, structs, globals)
- Imported module top-level code runs once, before the entry module

Not implemented yet:
- `import utils` followed by `utils.add(1, 2)` (module-qualified access)
- pub on let / const
- run-tree (tree-walking interpreter) with imports
- Subdirectory packages

Known limitation:
- Default parameter values are compiled at the call site. A default that uses a global variable of an imported module may resolve wrongly. Literal defaults (numbers, strings) are fine.
