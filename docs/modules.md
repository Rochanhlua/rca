# RCA Modules

Status: implemented for user modules (section 7) and standard library
modules (section 8). Some parts of the original design are still missing;
they are listed in section 7.

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
- `pub fn` and `pub struct` can be imported (see `enum.md` for `pub enum`).
- `pub let` and `pub const` are part of the design but are not implemented
  yet (`pub let` is a parse error).
- Importing a private name is a compile-time error.

## 4. Rules
- Each module is loaded and checked once.
- Circular imports are a compile-time error.
- A missing module file is a compile-time error with source location.
- Top-level code of an imported module does not run twice.

## 5. Not in this milestone
- Subdirectories / packages (`import a.b`)
- Package manager

## 6. Implementation order
1. Parser: import / from / as / pub
2. Semantic: module loading, name resolution, visibility
3. IR / VM: run imported modules
4. Tests and examples after each step

## 7. Implementation status (user modules)

Implemented:
- Parsing: import, import as, from-import, from-import as, pub fn, pub struct
- Module loader: finds NAME.rca next to the importing file, detects missing modules and circular imports, loads each module once
- Checker: from-import of pub functions and structs, private/missing name errors, type checking of imported calls
- rcac check and rcac run work across files
- Same names in different modules do not clash (functions, structs, globals)
- Imported module top-level code runs once, before the entry module
- Module-qualified calls: utils.add(1, 2), u.add(1, 2) after import utils as u, utils.Point(x = 1)

Not implemented yet:
- pub on let / const
- run-tree (tree-walking interpreter) with imports: `rcac run-tree` stops with
  "run-tree does not support import yet; use 'rca run'"
- Subdirectory packages

Known limitation:
- Default parameter values are compiled at the call site. A default that uses a global variable of an imported module may resolve wrongly. Literal defaults (numbers, strings) are fine.

## 8. Standard library modules

`import <module>` makes the functions of a standard library module
available with a dot, for example:

    import math
    import string

    print(math.pow(2, 10))
    print(string.trim("  hi  "))

The module loader does not look for a file for these names. The checker and
the IR lowering redirect `module.name(...)` to the underlying built-in
function, so a standard library call behaves exactly like the plain
built-in. There are 14 modules:

| Module | Functions (module name -> built-in) |
|---|---|
| `math` | abs, pow, floor, ceil, round |
| `fs` | read (read_file), write (write_file), exists (file_exists) |
| `time` | now, sleep |
| `string` | trim, split, replace |
| `json` | parse (json_parse), stringify (json_stringify) |
| `tensor` | matmul, zeros, ones, shape, reshape, transpose, sum, mean, max, min |
| `matrix` | matmul, transpose |
| `ai` | track, grad |
| `ml` | mse, sgd |
| `nn` | linear, relu, sigmoid, tanh, softmax, conv2d |
| `net` | connect (tcp_connect), send (tcp_send), recv (tcp_recv), close (tcp_close), udp_bind, udp_send, udp_recv, udp_close |
| `http` | get (http_get), post (http_post) |
| `crypto` | sha256 |
| `collections` | len, push, map_new, map_get, map_set, map_has, map_len |

The registry lives in `src/compiler/stdlib/mod.rs`.
`examples/stdlib_modules.rca` and `examples/stdlib_all_modules.rca` show
the import form.
