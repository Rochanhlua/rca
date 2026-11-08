use crate::compiler::lexer::token::Span;

#[derive(Debug, Clone, PartialEq)]
pub struct TypeExpr {
    pub name: String,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub enum BinaryOp {
    Add,
    Sub,
    Mul,
    Div,
    Mod,
    Eq,
    NotEq,
    Lt,
    LtEq,
    Gt,
    GtEq,
    And,
    Or,
    Range,
}

#[derive(Debug, Clone, PartialEq)]
pub enum UnaryOp {
    Neg,
    Not,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Argument {
    pub name: Option<String>,
    pub value: Expr,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Expr {
    pub kind: ExprKind,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ExprKind {
    Int(i64),
    Float(f64),
    Str(String),
    Bool(bool),
    NoneLit,
    Ident(String),
    Unary {
        op: UnaryOp,
        operand: Box<Expr>,
    },
    Binary {
        op: BinaryOp,
        left: Box<Expr>,
        right: Box<Expr>,
    },
    Call {
        callee: Box<Expr>,
        args: Vec<Argument>,
    },
    Field {
        object: Box<Expr>,
        name: String,
    },
    Index {
        object: Box<Expr>,
        index: Box<Expr>,
    },
    List(Vec<Expr>),
}

#[derive(Debug, Clone, PartialEq)]
pub struct Param {
    pub name: String,
    pub ty: TypeExpr,
    pub default: Option<Expr>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct FunctionDecl {
    pub name: String,
    pub params: Vec<Param>,
    pub return_type: Option<TypeExpr>,
    pub body: Vec<Stmt>,
    pub public: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct FieldDecl {
    pub name: String,
    pub ty: TypeExpr,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub struct StructDecl {
    pub name: String,
    pub fields: Vec<FieldDecl>,
    pub public: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Stmt {
    pub kind: StmtKind,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub enum StmtKind {
    Let {
        name: String,
        mutable: bool,
        ty: Option<TypeExpr>,
        value: Expr,
    },
    Assign {
        target: Expr,
        value: Expr,
    },
    Expr(Expr),
    Return(Option<Expr>),
    Break,
    Continue,
    If {
        condition: Expr,
        then_block: Vec<Stmt>,
        else_block: Option<Vec<Stmt>>,
    },
    While {
        condition: Expr,
        body: Vec<Stmt>,
    },
    For {
        var: String,
        iter: Expr,
        body: Vec<Stmt>,
    },
    Function(FunctionDecl),
    Struct(StructDecl),
    Enum(EnumDecl),
    Match {
        subject: Expr,
        arms: Vec<MatchArm>,
    },
    Try {
        body: Vec<Stmt>,
        catch_var: String,
        catch_block: Vec<Stmt>,
    },
    Throw(Expr),
    Import {
        module: String,
        alias: Option<String>,
    },
    FromImport {
        module: String,
        names: Vec<ImportName>,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub struct Program {
    pub statements: Vec<Stmt>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ImportName {
    pub name: String,
    pub alias: Option<String>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub struct EnumVariant {
    pub name: String,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub struct EnumDecl {
    pub name: String,
    pub variants: Vec<EnumVariant>,
    pub public: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Pattern {
    /// `_` matches anything.
    Wildcard,
    /// A literal or an enum variant such as `0`, `"a"`, `true`, `Color.Red`.
    Value(Expr),
}

#[derive(Debug, Clone, PartialEq)]
pub struct MatchArm {
    pub pattern: Pattern,
    pub body: Vec<Stmt>,
    pub span: Span,
}

/// Rewrites `match` into plain statements:
/// `if true: let $m = subject; if $m == p1: ... else: if $m == p2: ... else: ...`
/// The compiler and the interpreter both run this form, so they cannot disagree.
pub fn desugar_match(subject: &Expr, arms: &[MatchArm], span: &Span) -> Stmt {
    let hidden = format!("$match_{}_{}", span.line, span.column);
    let mk = |kind: ExprKind| Expr {
        kind,
        span: span.clone(),
    };
    let mut chain: Option<Vec<Stmt>> = None;
    for arm in arms.iter().rev() {
        let condition = match &arm.pattern {
            Pattern::Wildcard => mk(ExprKind::Bool(true)),
            Pattern::Value(p) => mk(ExprKind::Binary {
                op: BinaryOp::Eq,
                left: Box::new(mk(ExprKind::Ident(hidden.clone()))),
                right: Box::new(p.clone()),
            }),
        };
        let stmt = Stmt {
            kind: StmtKind::If {
                condition,
                then_block: arm.body.clone(),
                else_block: chain.take(),
            },
            span: arm.span.clone(),
        };
        chain = Some(vec![stmt]);
    }
    let mut body = vec![Stmt {
        kind: StmtKind::Let {
            name: hidden,
            mutable: false,
            ty: None,
            value: subject.clone(),
        },
        span: span.clone(),
    }];
    body.extend(chain.unwrap_or_default());
    Stmt {
        kind: StmtKind::If {
            condition: mk(ExprKind::Bool(true)),
            then_block: body,
            else_block: None,
        },
        span: span.clone(),
    }
}
