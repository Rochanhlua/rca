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
    (
        "tensor",
        &[
            ("matmul", "matmul"),
            ("zeros", "zeros"),
            ("ones", "ones"),
            ("shape", "shape"),
            ("reshape", "reshape"),
            ("transpose", "transpose"),
            ("sum", "sum"),
            ("mean", "mean"),
            ("max", "max"),
            ("min", "min"),
        ],
    ),
    (
        "matrix",
        &[("matmul", "matmul"), ("transpose", "transpose")],
    ),
    ("ai", &[("track", "track"), ("grad", "grad")]),
    ("ml", &[("mse", "mse"), ("sgd", "sgd")]),
    (
        "nn",
        &[
            ("linear", "linear"),
            ("relu", "relu"),
            ("sigmoid", "sigmoid"),
            ("tanh", "tanh"),
            ("softmax", "softmax"),
            ("conv2d", "conv2d"),
        ],
    ),
    (
        "net",
        &[
            ("connect", "tcp_connect"),
            ("send", "tcp_send"),
            ("recv", "tcp_recv"),
            ("close", "tcp_close"),
            ("udp_bind", "udp_bind"),
            ("udp_send", "udp_send"),
            ("udp_recv", "udp_recv"),
            ("udp_close", "udp_close"),
        ],
    ),
    ("http", &[("get", "http_get"), ("post", "http_post")]),
    ("crypto", &[("sha256", "sha256")]),
    (
        "collections",
        &[
            ("len", "len"),
            ("push", "push"),
            ("map_new", "map_new"),
            ("map_get", "map_get"),
            ("map_set", "map_set"),
            ("map_has", "map_has"),
            ("map_len", "map_len"),
        ],
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
