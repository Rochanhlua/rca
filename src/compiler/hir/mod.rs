//! HIR — high-level intermediate representation (Phase 3 §5 step 2).
//!
//! Mirror of the AST with a resolved `Type` attached to every expression.
//! The checker's `expr_types` map (step 1) supplies the type for each
//! expression; missing entries default to `Type::Unknown`.
//!
//! This module defines the node types and expression lowering only. Statement
//! lowering (`lower_ast_to_hir`) and the rewrite of `ir/lower.rs` to consume
//! HIR are steps 3 and 4.

use crate::compiler::ast::{
    BinaryOp, EnumDecl, Expr, ExprKind, FunctionDecl, ImportName, MatchArm, Param, Pattern,
    Program, Stmt, StmtKind, StructDecl, TypeExpr, UnaryOp,
};
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
    Char(char),
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
        ExprKind::Char(c) => HExprKind::Char(*c),
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

// ---------- statements ----------

#[derive(Debug, Clone)]
pub struct HStmt {
    pub kind: HStmtKind,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub enum HStmtKind {
    Let {
        name: String,
        mutable: bool,
        constant: bool,
        ty: Option<TypeExpr>,
        value: HExpr,
    },
    Assign {
        target: HExpr,
        value: HExpr,
    },
    Expr(HExpr),
    Return(Option<HExpr>),
    Break,
    Continue,
    If {
        condition: HExpr,
        then_block: Vec<HStmt>,
        else_block: Option<Vec<HStmt>>,
    },
    While {
        condition: HExpr,
        body: Vec<HStmt>,
    },
    For {
        var: String,
        iter: HExpr,
        body: Vec<HStmt>,
    },
    Function(HFunction),
    Struct(HStruct),
    Enum(HEnum),
    LetTuple {
        names: Vec<String>,
        mutable: bool,
        value: HExpr,
    },
    Match {
        subject: HExpr,
        arms: Vec<HMatchArm>,
    },
    Try {
        body: Vec<HStmt>,
        catch_var: String,
        catch_block: Vec<HStmt>,
    },
    Throw(HExpr),
    Import {
        module: String,
        alias: Option<String>,
    },
    FromImport {
        module: String,
        names: Vec<HImportName>,
    },
}

#[derive(Debug, Clone)]
pub struct HFunction {
    pub name: String,
    pub params: Vec<HParam>,
    pub return_type: Option<TypeExpr>,
    pub body: Vec<HStmt>,
    pub public: bool,
}

#[derive(Debug, Clone)]
pub struct HParam {
    pub name: String,
    pub ty: TypeExpr,
    pub default: Option<HExpr>,
}

#[derive(Debug, Clone)]
pub struct HStruct {
    pub name: String,
    pub fields: Vec<HField>,
    pub public: bool,
}

#[derive(Debug, Clone)]
pub struct HField {
    pub name: String,
    pub ty: TypeExpr,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct HEnum {
    pub name: String,
    pub variants: Vec<HVariant>,
    pub public: bool,
}

#[derive(Debug, Clone)]
pub struct HVariant {
    pub name: String,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub enum HPattern {
    Wildcard,
    Value(HExpr),
}

#[derive(Debug, Clone)]
pub struct HMatchArm {
    pub pattern: HPattern,
    pub body: Vec<HStmt>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct HImportName {
    pub name: String,
    pub alias: Option<String>,
    pub span: Span,
}

/// Lower every top-level AST statement to HIR.
pub fn lower_ast_to_hir(program: &Program, types: &HashMap<(usize, usize), Type>) -> Vec<HStmt> {
    program
        .statements
        .iter()
        .map(|s| lower_stmt(s, types))
        .collect()
}

fn lower_block(stmts: &[Stmt], types: &HashMap<(usize, usize), Type>) -> Vec<HStmt> {
    stmts.iter().map(|s| lower_stmt(s, types)).collect()
}

fn lower_fn_decl(f: &FunctionDecl, types: &HashMap<(usize, usize), Type>) -> HFunction {
    HFunction {
        name: f.name.clone(),
        params: f
            .params
            .iter()
            .map(|p: &Param| HParam {
                name: p.name.clone(),
                ty: p.ty.clone(),
                default: p.default.as_ref().map(|e| lower_expr(e, types)),
            })
            .collect(),
        return_type: f.return_type.clone(),
        body: lower_block(&f.body, types),
        public: f.public,
    }
}

fn lower_struct_decl(s: &StructDecl) -> HStruct {
    HStruct {
        name: s.name.clone(),
        fields: s
            .fields
            .iter()
            .map(|f| HField {
                name: f.name.clone(),
                ty: f.ty.clone(),
                span: f.span.clone(),
            })
            .collect(),
        public: s.public,
    }
}

fn lower_enum_decl(e: &EnumDecl) -> HEnum {
    HEnum {
        name: e.name.clone(),
        variants: e
            .variants
            .iter()
            .map(|v| HVariant {
                name: v.name.clone(),
                span: v.span.clone(),
            })
            .collect(),
        public: e.public,
    }
}

fn lower_match_arm(arm: &MatchArm, types: &HashMap<(usize, usize), Type>) -> HMatchArm {
    let pattern = match &arm.pattern {
        Pattern::Wildcard => HPattern::Wildcard,
        Pattern::Value(e) => HPattern::Value(lower_expr(e, types)),
    };
    HMatchArm {
        pattern,
        body: lower_block(&arm.body, types),
        span: arm.span.clone(),
    }
}

fn lower_import_name(n: &ImportName) -> HImportName {
    HImportName {
        name: n.name.clone(),
        alias: n.alias.clone(),
        span: n.span.clone(),
    }
}

pub fn lower_stmt(stmt: &Stmt, types: &HashMap<(usize, usize), Type>) -> HStmt {
    let kind = match &stmt.kind {
        StmtKind::Let {
            name,
            mutable,
            constant,
            ty,
            value,
        } => HStmtKind::Let {
            name: name.clone(),
            mutable: *mutable,
            constant: *constant,
            ty: ty.clone(),
            value: lower_expr(value, types),
        },
        StmtKind::Assign { target, value } => HStmtKind::Assign {
            target: lower_expr(target, types),
            value: lower_expr(value, types),
        },
        StmtKind::Expr(e) => HStmtKind::Expr(lower_expr(e, types)),
        StmtKind::Return(Some(e)) => HStmtKind::Return(Some(lower_expr(e, types))),
        StmtKind::Return(None) => HStmtKind::Return(None),
        StmtKind::Break => HStmtKind::Break,
        StmtKind::Continue => HStmtKind::Continue,
        StmtKind::If {
            condition,
            then_block,
            else_block,
        } => HStmtKind::If {
            condition: lower_expr(condition, types),
            then_block: lower_block(then_block, types),
            else_block: else_block.as_ref().map(|b| lower_block(b, types)),
        },
        StmtKind::While { condition, body } => HStmtKind::While {
            condition: lower_expr(condition, types),
            body: lower_block(body, types),
        },
        StmtKind::For { var, iter, body } => HStmtKind::For {
            var: var.clone(),
            iter: lower_expr(iter, types),
            body: lower_block(body, types),
        },
        StmtKind::Function(f) => HStmtKind::Function(lower_fn_decl(f, types)),
        StmtKind::Struct(s) => HStmtKind::Struct(lower_struct_decl(s)),
        StmtKind::Enum(e) => HStmtKind::Enum(lower_enum_decl(e)),
        StmtKind::LetTuple {
            names,
            mutable,
            value,
        } => HStmtKind::LetTuple {
            names: names.clone(),
            mutable: *mutable,
            value: lower_expr(value, types),
        },
        StmtKind::Match { subject, arms } => HStmtKind::Match {
            subject: lower_expr(subject, types),
            arms: arms.iter().map(|a| lower_match_arm(a, types)).collect(),
        },
        StmtKind::Try {
            body,
            catch_var,
            catch_block,
        } => HStmtKind::Try {
            body: lower_block(body, types),
            catch_var: catch_var.clone(),
            catch_block: lower_block(catch_block, types),
        },
        StmtKind::Throw(e) => HStmtKind::Throw(lower_expr(e, types)),
        StmtKind::Import { module, alias } => HStmtKind::Import {
            module: module.clone(),
            alias: alias.clone(),
        },
        StmtKind::FromImport { module, names } => HStmtKind::FromImport {
            module: module.clone(),
            names: names.iter().map(lower_import_name).collect(),
        },
    };
    HStmt {
        kind,
        span: stmt.span.clone(),
    }
}

// ---------- HIR -> AST (round-trip, phase 3 step 4a) ----------

use crate::compiler::ast::{Argument, EnumVariant, FieldDecl};

pub fn lower_hir_to_ast(stmts: &[HStmt]) -> Program {
    Program {
        statements: stmts.iter().map(lower_hstmt_to_ast).collect(),
    }
}

fn lower_block_to_ast(block: &[HStmt]) -> Vec<Stmt> {
    block.iter().map(lower_hstmt_to_ast).collect()
}

pub fn lower_hexpr_to_aexpr(h: &HExpr) -> Expr {
    let kind = match &h.kind {
        HExprKind::Int(n) => ExprKind::Int(*n),
        HExprKind::Float(f) => ExprKind::Float(*f),
        HExprKind::Str(s) => ExprKind::Str(s.clone()),
        HExprKind::Char(c) => ExprKind::Char(*c),
        HExprKind::Bool(b) => ExprKind::Bool(*b),
        HExprKind::NoneLit => ExprKind::NoneLit,
        HExprKind::Ident(n) => ExprKind::Ident(n.clone()),
        HExprKind::Unary { op, operand } => ExprKind::Unary {
            op: op.clone(),
            operand: Box::new(lower_hexpr_to_aexpr(operand)),
        },
        HExprKind::Binary { op, left, right } => ExprKind::Binary {
            op: op.clone(),
            left: Box::new(lower_hexpr_to_aexpr(left)),
            right: Box::new(lower_hexpr_to_aexpr(right)),
        },
        HExprKind::Call { callee, args } => ExprKind::Call {
            callee: Box::new(lower_hexpr_to_aexpr(callee)),
            args: args
                .iter()
                .map(|a| Argument {
                    name: a.name.clone(),
                    value: lower_hexpr_to_aexpr(&a.value),
                })
                .collect(),
        },
        HExprKind::Field { object, name } => ExprKind::Field {
            object: Box::new(lower_hexpr_to_aexpr(object)),
            name: name.clone(),
        },
        HExprKind::Index { object, index } => ExprKind::Index {
            object: Box::new(lower_hexpr_to_aexpr(object)),
            index: Box::new(lower_hexpr_to_aexpr(index)),
        },
        HExprKind::List(items) => ExprKind::List(items.iter().map(lower_hexpr_to_aexpr).collect()),
        HExprKind::Tuple(items) => {
            ExprKind::Tuple(items.iter().map(lower_hexpr_to_aexpr).collect())
        }
    };
    Expr {
        kind,
        span: h.span.clone(),
    }
}

pub fn lower_hstmt_to_ast(h: &HStmt) -> Stmt {
    let kind = match &h.kind {
        HStmtKind::Let {
            name,
            mutable,
            constant,
            ty,
            value,
        } => StmtKind::Let {
            name: name.clone(),
            mutable: *mutable,
            constant: *constant,
            ty: ty.clone(),
            value: lower_hexpr_to_aexpr(value),
        },
        HStmtKind::Assign { target, value } => StmtKind::Assign {
            target: lower_hexpr_to_aexpr(target),
            value: lower_hexpr_to_aexpr(value),
        },
        HStmtKind::Expr(e) => StmtKind::Expr(lower_hexpr_to_aexpr(e)),
        HStmtKind::Return(Some(e)) => StmtKind::Return(Some(lower_hexpr_to_aexpr(e))),
        HStmtKind::Return(None) => StmtKind::Return(None),
        HStmtKind::Break => StmtKind::Break,
        HStmtKind::Continue => StmtKind::Continue,
        HStmtKind::If {
            condition,
            then_block,
            else_block,
        } => StmtKind::If {
            condition: lower_hexpr_to_aexpr(condition),
            then_block: lower_block_to_ast(then_block),
            else_block: else_block.as_ref().map(|b| lower_block_to_ast(b)),
        },
        HStmtKind::While { condition, body } => StmtKind::While {
            condition: lower_hexpr_to_aexpr(condition),
            body: lower_block_to_ast(body),
        },
        HStmtKind::For { var, iter, body } => StmtKind::For {
            var: var.clone(),
            iter: lower_hexpr_to_aexpr(iter),
            body: lower_block_to_ast(body),
        },
        HStmtKind::Function(f) => StmtKind::Function(FunctionDecl {
            name: f.name.clone(),
            params: f
                .params
                .iter()
                .map(|p| Param {
                    name: p.name.clone(),
                    ty: p.ty.clone(),
                    default: p.default.as_ref().map(lower_hexpr_to_aexpr),
                })
                .collect(),
            return_type: f.return_type.clone(),
            body: lower_block_to_ast(&f.body),
            public: f.public,
        }),
        HStmtKind::Struct(s) => StmtKind::Struct(StructDecl {
            name: s.name.clone(),
            fields: s
                .fields
                .iter()
                .map(|f| FieldDecl {
                    name: f.name.clone(),
                    ty: f.ty.clone(),
                    span: f.span.clone(),
                })
                .collect(),
            public: s.public,
        }),
        HStmtKind::Enum(e) => StmtKind::Enum(EnumDecl {
            name: e.name.clone(),
            variants: e
                .variants
                .iter()
                .map(|v| EnumVariant {
                    name: v.name.clone(),
                    span: v.span.clone(),
                })
                .collect(),
            public: e.public,
        }),
        HStmtKind::LetTuple {
            names,
            mutable,
            value,
        } => StmtKind::LetTuple {
            names: names.clone(),
            mutable: *mutable,
            value: lower_hexpr_to_aexpr(value),
        },
        HStmtKind::Match { subject, arms } => StmtKind::Match {
            subject: lower_hexpr_to_aexpr(subject),
            arms: arms
                .iter()
                .map(|a| MatchArm {
                    pattern: match &a.pattern {
                        HPattern::Wildcard => Pattern::Wildcard,
                        HPattern::Value(e) => Pattern::Value(lower_hexpr_to_aexpr(e)),
                    },
                    body: lower_block_to_ast(&a.body),
                    span: a.span.clone(),
                })
                .collect(),
        },
        HStmtKind::Try {
            body,
            catch_var,
            catch_block,
        } => StmtKind::Try {
            body: lower_block_to_ast(body),
            catch_var: catch_var.clone(),
            catch_block: lower_block_to_ast(catch_block),
        },
        HStmtKind::Throw(e) => StmtKind::Throw(lower_hexpr_to_aexpr(e)),
        HStmtKind::Import { module, alias } => StmtKind::Import {
            module: module.clone(),
            alias: alias.clone(),
        },
        HStmtKind::FromImport { module, names } => StmtKind::FromImport {
            module: module.clone(),
            names: names
                .iter()
                .map(|n| ImportName {
                    name: n.name.clone(),
                    alias: n.alias.clone(),
                    span: n.span.clone(),
                })
                .collect(),
        },
    };
    Stmt {
        kind,
        span: h.span.clone(),
    }
}
