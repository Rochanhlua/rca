//! Markdown API documentation generator (dev tool #6).
//!
//! Scope:
//! - Emits Markdown to stdout from a single `.rca` file.
//! - Covers `pub fn`, `pub struct`, `pub enum` at the top level.
//! - Captures doc comments written as `#` lines immediately above the item.
//! - Signatures rendered from the AST (types re-rendered from `TypeExpr`).
//!
//! Not implemented yet: HTML output, private items, constants, module-level
//! cross-file docs, cross-references, anchors, or a config file.

use crate::compiler::ast::{EnumDecl, FunctionDecl, Program, StmtKind, StructDecl, TypeExpr};

pub fn generate(source: &str, program: &Program) -> String {
    let lines: Vec<&str> = source.lines().collect();
    let mut out = String::new();
    out.push_str("# RCA API Documentation\n\n");
    let mut emitted = 0usize;
    for stmt in &program.statements {
        match &stmt.kind {
            StmtKind::Function(f) if f.public => {
                emit_fn(&mut out, f, stmt.span.line, &lines);
                emitted += 1;
            }
            StmtKind::Struct(s) if s.public => {
                emit_struct(&mut out, s, stmt.span.line, &lines);
                emitted += 1;
            }
            StmtKind::Enum(e) if e.public => {
                emit_enum(&mut out, e, stmt.span.line, &lines);
                emitted += 1;
            }
            _ => {}
        }
    }
    if emitted == 0 {
        out.push_str("_No public items found._\n");
    }
    out
}

fn doc_comment_above(lines: &[&str], item_line_1based: usize) -> Vec<String> {
    let mut out = Vec::new();
    if item_line_1based < 2 {
        return out;
    }
    let mut idx = item_line_1based - 2;
    loop {
        let trimmed = lines[idx].trim();
        if trimmed.starts_with('#') {
            out.push(trimmed.trim_start_matches('#').trim().to_string());
            if idx == 0 {
                break;
            }
            idx -= 1;
        } else {
            break;
        }
    }
    out.reverse();
    out
}

fn emit_fn(out: &mut String, f: &FunctionDecl, line: usize, lines: &[&str]) {
    out.push_str(&format!("### {}\n\n", f.name));
    for c in doc_comment_above(lines, line) {
        out.push_str(&c);
        out.push('\n');
    }
    if !out.ends_with("\n\n") {
        out.push('\n');
    }
    out.push_str("```\n");
    out.push_str(&render_fn_sig(f));
    out.push_str("\n```\n\n");
}

fn render_fn_sig(f: &FunctionDecl) -> String {
    let params = f
        .params
        .iter()
        .map(|p| format!("{}: {}", p.name, type_str(&p.ty)))
        .collect::<Vec<_>>()
        .join(", ");
    let ret = f
        .return_type
        .as_ref()
        .map(|t| format!(" -> {}", type_str(t)))
        .unwrap_or_default();
    format!("fn {}({}){}", f.name, params, ret)
}

fn type_str(t: &TypeExpr) -> String {
    if t.elements.is_empty() {
        t.name.clone()
    } else {
        format!(
            "({})",
            t.elements
                .iter()
                .map(type_str)
                .collect::<Vec<_>>()
                .join(", ")
        )
    }
}

fn emit_struct(out: &mut String, s: &StructDecl, line: usize, lines: &[&str]) {
    out.push_str(&format!("### {}\n\n", s.name));
    for c in doc_comment_above(lines, line) {
        out.push_str(&c);
        out.push('\n');
    }
    if !out.ends_with("\n\n") {
        out.push('\n');
    }
    out.push_str("```\n");
    out.push_str(&format!("struct {}\n", s.name));
    for f in &s.fields {
        out.push_str(&format!("    {}: {}\n", f.name, type_str(&f.ty)));
    }
    out.push_str("```\n\n");
}

fn emit_enum(out: &mut String, e: &EnumDecl, line: usize, lines: &[&str]) {
    out.push_str(&format!("### {}\n\n", e.name));
    for c in doc_comment_above(lines, line) {
        out.push_str(&c);
        out.push('\n');
    }
    if !out.ends_with("\n\n") {
        out.push('\n');
    }
    out.push_str("```\n");
    out.push_str(&format!("enum {}\n", e.name));
    for v in &e.variants {
        out.push_str(&format!("    {}\n", v.name));
    }
    out.push_str("```\n\n");
}
