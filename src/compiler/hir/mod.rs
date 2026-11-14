//! HIR — high-level intermediate representation (Phase 3 §5 step 2).
//!
//! Mirror of the AST with a resolved `Type` attached to every expression.
//! The checker's `expr_types` map (step 1) supplies the type for each
//! expression; missing entries default to `Type::Unknown`.
//!
//! This module defines the node types and expression lowering only. Statement
//! lowering (`lower_ast_to_hir`) and the rewrite of `ir/lower.rs` to consume
//! HIR are steps 3 and 4.

use crate::compiler::ast::{BinaryOp, Expr, ExprKind, UnaryOp};
use crate::compiler::lexer::token::Span;
use crate::compiler::types::Type;
use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct HExpr {
    pub kind: HExprKind,
    pub span: Span,
    pub ty: Type,
}

#[derive(Debug, Clone)]
pub enum HExprKind {
    Int(i64),
    Float(f64),
    Str(String),
    Bool(bool),
    NoneLit,
    Ident(String),
    Unary {
        op: UnaryOp,
        operand: Box<HExpr>,
    },
    Binary {
        op: BinaryOp,
        left: Box<HExpr>,
        right: Box<HExpr>,
    },
    Call {
        callee: Box<HExpr>,
        args: Vec<HArgument>,
    },
    Field {
        object: Box<HExpr>,
        name: String,
    },
    Index {
        object: Box<HExpr>,
        index: Box<HExpr>,
    },
    List(Vec<HExpr>),
    Tuple(Vec<HExpr>),
}

#[derive(Debug, Clone)]
pub struct HArgument {
    pub name: Option<String>,
    pub value: HExpr,
}

/// Lower a single AST expression to HIR, attaching the checker's recorded
/// type (or `Type::Unknown` if the map has no entry for this span).
pub fn lower_expr(expr: &Expr, types: &HashMap<(usize, usize), Type>) -> HExpr {
    let kind = match &expr.kind {
        ExprKind::Int(n) => HExprKind::Int(*n),
        ExprKind::Float(f) => HExprKind::Float(*f),
        ExprKind::Str(s) => HExprKind::Str(s.clone()),
        ExprKind::Bool(b) => HExprKind::Bool(*b),
        ExprKind::NoneLit => HExprKind::NoneLit,
        ExprKind::Ident(n) => HExprKind::Ident(n.clone()),
        ExprKind::Unary { op, operand } => HExprKind::Unary {
            op: op.clone(),
            operand: Box::new(lower_expr(operand, types)),
        },
        ExprKind::Binary { op, left, right } => HExprKind::Binary {
            op: op.clone(),
            left: Box::new(lower_expr(left, types)),
            right: Box::new(lower_expr(right, types)),
        },
        ExprKind::Call { callee, args } => HExprKind::Call {
            callee: Box::new(lower_expr(callee, types)),
            args: args
                .iter()
                .map(|a| HArgument {
                    name: a.name.clone(),
                    value: lower_expr(&a.value, types),
                })
                .collect(),
        },
        ExprKind::Field { object, name } => HExprKind::Field {
            object: Box::new(lower_expr(object, types)),
            name: name.clone(),
        },
        ExprKind::Index { object, index } => HExprKind::Index {
            object: Box::new(lower_expr(object, types)),
            index: Box::new(lower_expr(index, types)),
        },
        ExprKind::List(items) => {
            HExprKind::List(items.iter().map(|e| lower_expr(e, types)).collect())
        }
        ExprKind::Tuple(items) => {
            HExprKind::Tuple(items.iter().map(|e| lower_expr(e, types)).collect())
        }
    };
    let ty = types
        .get(&(expr.span.line, expr.span.column))
        .cloned()
        .unwrap_or(Type::Unknown);
    HExpr {
        kind,
        span: expr.span.clone(),
        ty,
    }
}
