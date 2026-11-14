//! Minimal lint pass: unused variables and unreachable code.
//! First milestone of the developer-tooling track (§12 Phase 7).
//!
//! Scope (intentionally small):
//! - Unused `let` / `LetTuple` / `for` loop variables / `catch` variables
//!   per block. Function parameters are not checked yet.
//! - Unreachable statements after `return` / `break` / `continue` / `throw`
//!   in the same block (first one reported per block).
//!
//! Not yet implemented: shadowing, dead stores, unused functions, unused
//! imports, data-flow checks. Lint runs on the entry file's AST only;
//! imported modules are not linted.

use crate::compiler::ast::{Expr, ExprKind, Pattern, Program, Stmt, StmtKind};
use crate::compiler::diagnostics::Diagnostic;
use crate::compiler::lexer::token::Span;
use std::collections::HashSet;

pub fn lint_program(program: &Program) -> Vec<Diagnostic> {
    let mut warnings = Vec::new();
    lint_block(&program.statements, &mut warnings);
    warnings
}

fn lint_block(stmts: &[Stmt], warnings: &mut Vec<Diagnostic>) {
    // Unreachable after a terminating statement.
    let mut terminated = false;
    for s in stmts {
        if terminated {
            warnings.push(Diagnostic::new(
                "unreachable statement".to_string(),
                s.span.clone(),
            ));
            break;
        }
        if matches!(
            &s.kind,
            StmtKind::Return(_) | StmtKind::Break | StmtKind::Continue | StmtKind::Throw(_)
        ) {
            terminated = true;
        }
    }

    // Unused vars in this block.
    let mut declared: Vec<(String, Span)> = Vec::new();
    let mut used: HashSet<String> = HashSet::new();
    for s in stmts {
        collect_stmt(&mut declared, &mut used, s);
    }
    for (name, span) in declared {
        if !used.contains(&name) {
            warnings.push(Diagnostic::new(format!("unused variable '{}'", name), span));
        }
    }

    // Recurse into nested blocks.
    for s in stmts {
        recurse(s, warnings);
    }
}

fn recurse(s: &Stmt, warnings: &mut Vec<Diagnostic>) {
    match &s.kind {
        StmtKind::Function(f) => lint_block(&f.body, warnings),
        StmtKind::If {
            then_block,
            else_block,
            ..
        } => {
            lint_block(then_block, warnings);
            if let Some(eb) = else_block {
                lint_block(eb, warnings);
            }
        }
        StmtKind::While { body, .. } => lint_block(body, warnings),
        StmtKind::For { body, .. } => lint_block(body, warnings),
        StmtKind::Match { arms, .. } => {
            for arm in arms {
                lint_block(&arm.body, warnings);
            }
        }
        StmtKind::Try {
            body, catch_block, ..
        } => {
            lint_block(body, warnings);
            lint_block(catch_block, warnings);
        }
        _ => {}
    }
}

fn collect_stmt(declared: &mut Vec<(String, Span)>, used: &mut HashSet<String>, s: &Stmt) {
    match &s.kind {
        StmtKind::Let { name, value, .. } => {
            expr_uses(value, used);
            declared.push((name.clone(), s.span.clone()));
        }
        StmtKind::LetTuple { names, value, .. } => {
            expr_uses(value, used);
            for n in names {
                declared.push((n.clone(), s.span.clone()));
            }
        }
        StmtKind::For { var, iter, body } => {
            expr_uses(iter, used);
            declared.push((var.clone(), s.span.clone()));
            block_uses(body, used);
        }
        StmtKind::Try {
            body,
            catch_var,
            catch_block,
        } => {
            block_uses(body, used);
            declared.push((catch_var.clone(), s.span.clone()));
            block_uses(catch_block, used);
        }
        _ => stmt_uses(s, used),
    }
}

fn block_uses(stmts: &[Stmt], used: &mut HashSet<String>) {
    for s in stmts {
        stmt_uses(s, used);
    }
}

fn stmt_uses(s: &Stmt, used: &mut HashSet<String>) {
    match &s.kind {
        StmtKind::Let { value, .. } => expr_uses(value, used),
        StmtKind::LetTuple { value, .. } => expr_uses(value, used),
        StmtKind::Assign { target, value } => {
            expr_uses(target, used);
            expr_uses(value, used);
        }
        StmtKind::Expr(e) => expr_uses(e, used),
        StmtKind::Return(Some(e)) => expr_uses(e, used),
        StmtKind::Return(None) | StmtKind::Break | StmtKind::Continue => {}
        StmtKind::If {
            condition,
            then_block,
            else_block,
        } => {
            expr_uses(condition, used);
            block_uses(then_block, used);
            if let Some(eb) = else_block {
                block_uses(eb, used);
            }
        }
        StmtKind::While { condition, body } => {
            expr_uses(condition, used);
            block_uses(body, used);
        }
        StmtKind::For { iter, body, .. } => {
            expr_uses(iter, used);
            block_uses(body, used);
        }
        StmtKind::Function(f) => block_uses(&f.body, used),
        StmtKind::Match { subject, arms } => {
            expr_uses(subject, used);
            for arm in arms {
                if let Pattern::Value(e) = &arm.pattern {
                    expr_uses(e, used);
                }
                block_uses(&arm.body, used);
            }
        }
        StmtKind::Try {
            body, catch_block, ..
        } => {
            block_uses(body, used);
            block_uses(catch_block, used);
        }
        StmtKind::Throw(e) => expr_uses(e, used),
        StmtKind::Struct(_) | StmtKind::Enum(_) => {}
        StmtKind::Import { .. } | StmtKind::FromImport { .. } => {}
    }
}

fn expr_uses(e: &Expr, used: &mut HashSet<String>) {
    match &e.kind {
        ExprKind::Ident(name) => {
            used.insert(name.clone());
        }
        ExprKind::Int(_)
        | ExprKind::Float(_)
        | ExprKind::Str(_)
        | ExprKind::Bool(_)
        | ExprKind::NoneLit => {}
        ExprKind::Unary { operand, .. } => expr_uses(operand, used),
        ExprKind::Binary { left, right, .. } => {
            expr_uses(left, used);
            expr_uses(right, used);
        }
        ExprKind::Call { callee, args } => {
            expr_uses(callee, used);
            for a in args {
                expr_uses(&a.value, used);
            }
        }
        ExprKind::Field { object, .. } => expr_uses(object, used),
        ExprKind::Index { object, index } => {
            expr_uses(object, used);
            expr_uses(index, used);
        }
        ExprKind::List(items) | ExprKind::Tuple(items) => {
            for it in items {
                expr_uses(it, used);
            }
        }
    }
}
