//! Minimal Language Server Protocol (LSP) implementation (dev tool #5).
//!
//! Scope (deliberately small, first milestone):
//! - Transport: JSON-RPC 2.0 over stdin/stdout with `Content-Length` framing.
//! - Methods: `initialize`, `initialized`, `shutdown`, `exit`.
//! - Notifications handled: `textDocument/didOpen`, `textDocument/didChange`.
//!   On either, the document is parsed and checked; diagnostics are published
//!   via `textDocument/publishDiagnostics`.
//!
//! Uses the crate's existing `json` module (no `serde_json` dependency).
//!
//! Not implemented yet: hover, completion, go-to-definition, formatting,
//! semantic tokens, workspace symbols, rename, multi-document state.
//! Diagnostics come from the parser and checker only; the linter is not run.

use crate::compiler::lexer::Lexer;
use crate::compiler::parser::Parser;
use crate::compiler::semantic::Checker;
use crate::runtime::json;
use crate::runtime::values::Value;
use std::io::{self, BufRead, Read, Write};

pub fn run_lsp() -> i32 {
    let stdin = io::stdin();
    let mut reader = stdin.lock();
    let stdout = io::stdout();
    let mut writer = stdout.lock();

    loop {
        let msg = match read_message(&mut reader) {
            Ok(Some(m)) => m,
            Ok(None) => return 0,
            Err(e) => {
                eprintln!("lsp: read error: {}", e);
                return 1;
            }
        };
        let parsed = match json::parse(&msg) {
            Ok(v) => v,
            Err(e) => {
                eprintln!("lsp: invalid JSON: {}", e);
                continue;
            }
        };
        let (id, method, params) = match extract_request(&parsed) {
            Some(t) => t,
            None => continue,
        };
        match method.as_str() {
            "initialize" => {
                let result = Value::Map(vec![(
                    Value::Str("capabilities".into()),
                    Value::Map(vec![(
                        Value::Str("textDocumentSync".into()),
                        Value::Int(1), // full sync
                    )]),
                )]);
                if let Some(idv) = id {
                    let _ = write_response(&mut writer, idv, result);
                }
            }
            "initialized" => { /* notification, no response */ }
            "shutdown" => {
                if let Some(idv) = id {
                    let _ = write_response(&mut writer, idv, Value::None);
                }
            }
            "exit" => return 0,
            "textDocument/didOpen" | "textDocument/didChange" => {
                if let Some((uri, text)) = extract_document(&params) {
                    let diags = compute_diagnostics(&text);
                    let _ = write_publish_diagnostics(&mut writer, &uri, diags);
                }
            }
            _ => {
                // Unknown method; respond with error if it had an id.
                if let Some(idv) = id {
                    let err = Value::Map(vec![
                        (Value::Str("code".into()), Value::Int(-32601)),
                        (
                            Value::Str("message".into()),
                            Value::Str("method not found".into()),
                        ),
                    ]);
                    let _ = write_error(&mut writer, idv, err);
                }
            }
        }
    }
}

fn read_message<R: BufRead + Read>(r: &mut R) -> io::Result<Option<String>> {
    let mut content_length: Option<usize> = None;
    loop {
        let mut line = String::new();
        let n = r.read_line(&mut line)?;
        if n == 0 {
            return Ok(None); // EOF
        }
        let trimmed = line.trim_end_matches(['\r', '\n']);
        if trimmed.is_empty() {
            break; // blank line ends headers
        }
        if let Some(rest) = trimmed.strip_prefix("Content-Length:") {
            content_length = rest.trim().parse::<usize>().ok();
        }
    }
    let len = match content_length {
        Some(l) => l,
        None => return Ok(None),
    };
    let mut buf = vec![0u8; len];
    r.read_exact(&mut buf)?;
    Ok(Some(String::from_utf8_lossy(&buf).to_string()))
}

fn extract_request(v: &Value) -> Option<(Option<Value>, String, Value)> {
    let m = match v {
        Value::Map(m) => m,
        _ => return None,
    };
    let mut id: Option<Value> = None;
    let mut method = String::new();
    let mut params = Value::None;
    for (k, v) in m {
        if let Value::Str(k) = k {
            match k.as_str() {
                "id" => id = Some(v.clone()),
                "method" => {
                    if let Value::Str(s) = v {
                        method = s.clone();
                    }
                }
                "params" => params = v.clone(),
                _ => {}
            }
        }
    }
    if method.is_empty() {
        return None;
    }
    Some((id, method, params))
}

fn extract_document(params: &Value) -> Option<(String, String)> {
    let m = match params {
        Value::Map(m) => m,
        _ => return None,
    };
    let mut doc: Option<&Value> = None;
    for (k, v) in m {
        if let Value::Str(s) = k
            && s == "textDocument"
        {
            doc = Some(v);
        }
    }
    let doc = doc?;
    let dm = match doc {
        Value::Map(m) => m,
        _ => return None,
    };
    let mut uri = String::new();
    let mut text = String::new();
    for (k, v) in dm {
        if let Value::Str(s) = k {
            match s.as_str() {
                "uri" => {
                    if let Value::Str(u) = v {
                        uri = u.clone();
                    }
                }
                "text" => {
                    if let Value::Str(t) = v {
                        text = t.clone();
                    }
                }
                _ => {}
            }
        }
    }
    if uri.is_empty() {
        return None;
    }
    Some((uri, text))
}

fn compute_diagnostics(text: &str) -> Vec<Value> {
    let mut out = Vec::new();
    let tokens = match Lexer::new(text).tokenize() {
        Ok(t) => t,
        Err(d) => {
            out.push(diagnostic_json(d.span.line, d.span.column, &d.message, 1));
            return out;
        }
    };
    let program = match Parser::new(tokens).parse_program() {
        Ok(p) => p,
        Err(d) => {
            out.push(diagnostic_json(d.span.line, d.span.column, &d.message, 1));
            return out;
        }
    };
    if let Err(errs) = Checker::new().check_program(&program) {
        for d in errs {
            out.push(diagnostic_json(d.span.line, d.span.column, &d.message, 1));
        }
    }
    out
}

fn diagnostic_json(line: usize, column: usize, message: &str, severity: i64) -> Value {
    // LSP is 0-based; RCA spans are 1-based.
    let ln = line.saturating_sub(1) as i64;
    let ch = column.saturating_sub(1) as i64;
    let start = Value::Map(vec![
        (Value::Str("line".into()), Value::Int(ln)),
        (Value::Str("character".into()), Value::Int(ch)),
    ]);
    let end = Value::Map(vec![
        (Value::Str("line".into()), Value::Int(ln)),
        (Value::Str("character".into()), Value::Int(ch + 1)),
    ]);
    Value::Map(vec![
        (
            Value::Str("range".into()),
            Value::Map(vec![
                (Value::Str("start".into()), start),
                (Value::Str("end".into()), end),
            ]),
        ),
        (Value::Str("severity".into()), Value::Int(severity)),
        (Value::Str("source".into()), Value::Str("rcac".into())),
        (
            Value::Str("message".into()),
            Value::Str(message.to_string()),
        ),
    ])
}

fn write_frame<W: Write>(w: &mut W, body: &str) -> io::Result<()> {
    write!(w, "Content-Length: {}\r\n\r\n{}", body.len(), body)?;
    w.flush()
}

fn write_response<W: Write>(w: &mut W, id: Value, result: Value) -> io::Result<()> {
    let msg = Value::Map(vec![
        (Value::Str("jsonrpc".into()), Value::Str("2.0".into())),
        (Value::Str("id".into()), id),
        (Value::Str("result".into()), result),
    ]);
    let body = json::stringify(&msg).map_err(io::Error::other)?;
    write_frame(w, &body)
}

fn write_error<W: Write>(w: &mut W, id: Value, error: Value) -> io::Result<()> {
    let msg = Value::Map(vec![
        (Value::Str("jsonrpc".into()), Value::Str("2.0".into())),
        (Value::Str("id".into()), id),
        (Value::Str("error".into()), error),
    ]);
    let body = json::stringify(&msg).map_err(io::Error::other)?;
    write_frame(w, &body)
}

fn write_publish_diagnostics<W: Write>(w: &mut W, uri: &str, diags: Vec<Value>) -> io::Result<()> {
    let params = Value::Map(vec![
        (Value::Str("uri".into()), Value::Str(uri.to_string())),
        (Value::Str("diagnostics".into()), Value::List(diags)),
    ]);
    let msg = Value::Map(vec![
        (Value::Str("jsonrpc".into()), Value::Str("2.0".into())),
        (
            Value::Str("method".into()),
            Value::Str("textDocument/publishDiagnostics".into()),
        ),
        (Value::Str("params".into()), params),
    ]);
    let body = json::stringify(&msg).map_err(io::Error::other)?;
    write_frame(w, &body)
}
