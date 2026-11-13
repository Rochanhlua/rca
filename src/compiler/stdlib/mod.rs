//! Standard library modules (blueprint §8).

pub const MODULES: &[(&str, &[(&str, &str)])] = &[
    (
        "math",
        &[
            ("abs", "abs"),
            ("pow", "pow"),
            ("floor", "floor"),
            ("ceil", "ceil"),
            ("round", "round"),
        ],
    ),
    (
        "fs",
        &[
            ("read", "read_file"),
            ("write", "write_file"),
            ("exists", "file_exists"),
        ],
    ),
    ("time", &[("now", "now"), ("sleep", "sleep")]),
    (
        "string",
        &[("trim", "trim"), ("split", "split"), ("replace", "replace")],
    ),
    (
        "json",
        &[("parse", "json_parse"), ("stringify", "json_stringify")],
    ),
];

pub fn is_stdlib_module(name: &str) -> bool {
    MODULES.iter().any(|&(n, _)| n == name)
}

pub fn builtin_name(module: &str, item: &str) -> Option<String> {
    for &(n, items) in MODULES {
        if n == module {
            for &(i, b) in items {
                if i == item {
                    return Some(b.to_string());
                }
            }
        }
    }
    None
}
