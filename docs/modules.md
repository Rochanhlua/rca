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
