use crate::compiler::ast::*;
use crate::compiler::diagnostics::Diagnostic;
use crate::compiler::lexer::token::{Span, Token, TokenKind};

pub struct Parser {
    tokens: Vec<Token>,
    pos: usize,
}

impl Parser {
    pub fn new(tokens: Vec<Token>) -> Self {
        Parser { tokens, pos: 0 }
    }

    // ---------- helpers ----------

    fn peek(&self) -> &TokenKind {
        &self.tokens[self.pos].kind
    }

    fn peek_at(&self, offset: usize) -> &TokenKind {
        let i = (self.pos + offset).min(self.tokens.len() - 1);
        &self.tokens[i].kind
    }

    fn span(&self) -> Span {
        self.tokens[self.pos].span.clone()
    }

    fn advance(&mut self) -> Token {
        let t = self.tokens[self.pos].clone();
        if self.pos < self.tokens.len() - 1 {
            self.pos += 1;
        }
        t
    }

    fn check(&self, kind: &TokenKind) -> bool {
        self.peek() == kind
    }

    fn eat(&mut self, kind: &TokenKind) -> bool {
        if self.check(kind) {
            self.advance();
            true
        } else {
            false
        }
    }

    fn expect(&mut self, kind: &TokenKind, what: &str) -> Result<Token, Diagnostic> {
        if self.check(kind) {
            Ok(self.advance())
        } else {
            Err(Diagnostic::new(
                format!("expected {}, found {:?}", what, self.peek()),
                self.span(),
            ))
        }
    }

    fn expect_ident(&mut self, what: &str) -> Result<(String, Span), Diagnostic> {
        let span = self.span();
        match self.peek().clone() {
            TokenKind::Identifier(name) => {
                self.advance();
                Ok((name, span))
            }
            TokenKind::LParen if what == "a variable name" || what == "a parameter name" => {
                Err(Diagnostic::new(
                    format!(
                        "expected {}, found '(' (tuple patterns are not supported here)",
                        what
                    ),
                    span,
                ))
            }
            other => Err(Diagnostic::new(
                format!("expected {}, found {:?}", what, other),
                span,
            )),
        }
    }

    fn skip_newlines(&mut self) {
        while self.check(&TokenKind::Newline) {
            self.advance();
        }
    }

    fn end_simple(&mut self) -> Result<(), Diagnostic> {
        if self.check(&TokenKind::Newline) {
            self.advance();
            Ok(())
        } else if self.check(&TokenKind::Eof) {
            Ok(())
        } else {
            Err(Diagnostic::new(
                format!("expected end of line, found {:?}", self.peek()),
                self.span(),
            ))
        }
    }

    // ---------- program and statements ----------

    pub fn parse_program(&mut self) -> Result<Program, Diagnostic> {
        let mut statements = Vec::new();
        self.skip_newlines();
        while !self.check(&TokenKind::Eof) {
            statements.push(self.parse_statement()?);
            self.skip_newlines();
        }
        Ok(Program { statements })
    }

    fn parse_block(&mut self) -> Result<Vec<Stmt>, Diagnostic> {
        self.expect(&TokenKind::Colon, "':'")?;
        self.expect(&TokenKind::Newline, "a new line after ':'")?;
        self.expect(&TokenKind::Indent, "an indented block")?;
        let mut stmts = Vec::new();
        self.skip_newlines();
        while !self.check(&TokenKind::Dedent) && !self.check(&TokenKind::Eof) {
            stmts.push(self.parse_statement()?);
            self.skip_newlines();
        }
        self.expect(&TokenKind::Dedent, "end of block")?;
        Ok(stmts)
    }

    fn parse_statement(&mut self) -> Result<Stmt, Diagnostic> {
        let span = self.span();
        let kind = match self.peek().clone() {
            TokenKind::Let if self.peek_at(1) == &TokenKind::LParen => {
                self.parse_let_tuple(false)?
            }
            TokenKind::Mut if self.peek_at(1) == &TokenKind::LParen => {
                self.parse_let_tuple(true)?
            }
            TokenKind::Let => self.parse_let(false, false)?,
            TokenKind::Mut => self.parse_let(true, false)?,
            TokenKind::Const => self.parse_let(false, true)?,
            TokenKind::Fn => StmtKind::Function(self.parse_function()?),
            TokenKind::Struct => StmtKind::Struct(self.parse_struct()?),
            TokenKind::Enum => StmtKind::Enum(self.parse_enum()?),
            TokenKind::Match => self.parse_match()?,
            TokenKind::Pub => {
                self.advance(); // pub
                match self.peek().clone() {
                    TokenKind::Fn => {
                        let mut f = self.parse_function()?;
                        f.public = true;
                        StmtKind::Function(f)
                    }
                    TokenKind::Struct => {
                        let mut s = self.parse_struct()?;
                        s.public = true;
                        StmtKind::Struct(s)
                    }
                    TokenKind::Enum => {
                        let mut e = self.parse_enum()?;
                        e.public = true;
                        StmtKind::Enum(e)
                    }
                    other => {
                        return Err(Diagnostic::new(
                            format!("expected 'fn' or 'struct' after 'pub', found {:?}", other),
                            self.span(),
                        ));
                    }
                }
            }
            TokenKind::If => self.parse_if()?,
            TokenKind::While => self.parse_while()?,
            TokenKind::For => self.parse_for()?,
            TokenKind::Try => self.parse_try()?,
            TokenKind::Import => self.parse_import()?,
            TokenKind::From => self.parse_from_import()?,
            TokenKind::Throw => {
                self.advance();
                let value = self.parse_expr()?;
                self.end_simple()?;
                StmtKind::Throw(value)
            }
            TokenKind::Return => {
                self.advance();
                let value = if self.check(&TokenKind::Newline) || self.check(&TokenKind::Eof) {
                    None
                } else {
                    Some(self.parse_expr()?)
                };
                self.end_simple()?;
                StmtKind::Return(value)
            }
            TokenKind::Break => {
                self.advance();
                self.end_simple()?;
                StmtKind::Break
            }
            TokenKind::Continue => {
                self.advance();
                self.end_simple()?;
                StmtKind::Continue
            }
            _ => {
                let expr = self.parse_expr()?;
                if self.eat(&TokenKind::Assign) {
                    let value = self.parse_expr()?;
                    self.end_simple()?;
                    StmtKind::Assign {
                        target: expr,
                        value,
                    }
                } else {
                    self.end_simple()?;
                    StmtKind::Expr(expr)
                }
            }
        };
        Ok(Stmt { kind, span })
    }

    fn parse_import(&mut self) -> Result<StmtKind, Diagnostic> {
        self.advance(); // import
        let (module, _) = self.expect_ident("a module name")?;
        let alias = if self.eat(&TokenKind::As) {
            Some(self.expect_ident("an alias name")?.0)
        } else {
            None
        };
        self.end_simple()?;
        Ok(StmtKind::Import { module, alias })
    }

    fn parse_from_import(&mut self) -> Result<StmtKind, Diagnostic> {
        self.advance(); // from
        let (module, _) = self.expect_ident("a module name")?;
        self.expect(&TokenKind::Import, "'import'")?;
        let mut names = Vec::new();
        loop {
            let (name, span) = self.expect_ident("a name to import")?;
            let alias = if self.eat(&TokenKind::As) {
                Some(self.expect_ident("an alias name")?.0)
            } else {
                None
            };
            names.push(ImportName { name, alias, span });
            if !self.eat(&TokenKind::Comma) {
                break;
            }
        }
        self.end_simple()?;
        Ok(StmtKind::FromImport { module, names })
    }

    fn parse_type(&mut self) -> Result<TypeExpr, Diagnostic> {
        let span = self.span();
        if self.eat(&TokenKind::LParen) {
            let mut elements = vec![self.parse_type()?];
            while self.eat(&TokenKind::Comma) {
                if self.check(&TokenKind::RParen) {
                    break;
                }
                elements.push(self.parse_type()?);
            }
            self.expect(&TokenKind::RParen, "')'")?;
            if elements.len() < 2 {
                return Err(Diagnostic::new(
                    "a tuple type needs at least 2 elements",
                    span,
                ));
            }
            return Ok(TypeExpr {
                name: String::new(),
                elements,
                span,
            });
        }
        let (name, span) = self.expect_ident("a type name")?;
        Ok(TypeExpr {
            name,
            elements: Vec::new(),
            span,
        })
    }

    fn parse_let_tuple(&mut self, mutable: bool) -> Result<StmtKind, Diagnostic> {
        let span = self.span();
        self.advance(); // let or mut
        self.expect(&TokenKind::LParen, "'('")?;
        let mut names = Vec::new();
        loop {
            let (n, _) = self.expect_ident("a variable name")?;
            names.push(n);
            if !self.eat(&TokenKind::Comma) || self.check(&TokenKind::RParen) {
                break;
            }
        }
        self.expect(&TokenKind::RParen, "')'")?;
        if names.len() < 2 {
            return Err(Diagnostic::new(
                "a tuple pattern needs at least 2 names",
                span,
            ));
        }
        self.expect(&TokenKind::Assign, "'='")?;
        let value = self.parse_expr()?;
        self.end_simple()?;
        Ok(StmtKind::LetTuple {
            names,
            mutable,
            value,
        })
    }

    fn parse_let(&mut self, mutable: bool, constant: bool) -> Result<StmtKind, Diagnostic> {
        self.advance(); // let or mut
        let (name, _) = self.expect_ident("a variable name")?;
        let ty = if self.eat(&TokenKind::Colon) {
            Some(self.parse_type()?)
        } else {
            None
        };
        self.expect(&TokenKind::Assign, "'='")?;
        let value = self.parse_expr()?;
        self.end_simple()?;
        Ok(StmtKind::Let {
            name,
            mutable,
            constant,
            ty,
            value,
        })
    }

    fn parse_if(&mut self) -> Result<StmtKind, Diagnostic> {
        self.advance(); // if or elif
        let condition = self.parse_expr()?;
        let then_block = self.parse_block()?;
        let else_block = if self.check(&TokenKind::Elif) {
            let span = self.span();
            let nested = self.parse_if()?;
            Some(vec![Stmt { kind: nested, span }])
        } else if self.eat(&TokenKind::Else) {
            Some(self.parse_block()?)
        } else {
            None
        };
        Ok(StmtKind::If {
            condition,
            then_block,
            else_block,
        })
    }

    fn parse_match(&mut self) -> Result<StmtKind, Diagnostic> {
        self.advance(); // match
        let subject = self.parse_expr()?;
        self.expect(&TokenKind::Colon, "':'")?;
        self.expect(&TokenKind::Newline, "a new line after ':'")?;
        self.expect(&TokenKind::Indent, "an indented block")?;
        let mut arms = Vec::new();
        self.skip_newlines();
        while !self.check(&TokenKind::Dedent) && !self.check(&TokenKind::Eof) {
            let span = self.span();
            let pattern = if matches!(self.peek(), TokenKind::Identifier(n) if n == "_") {
                self.advance();
                Pattern::Wildcard
            } else {
                Pattern::Value(self.parse_expr()?)
            };
            let body = self.parse_block()?;
            arms.push(MatchArm {
                pattern,
                body,
                span,
            });
            self.skip_newlines();
        }
        let end_span = self.span();
        self.expect(&TokenKind::Dedent, "end of match")?;
        if arms.is_empty() {
            return Err(Diagnostic::new(
                "a match must have at least one arm",
                end_span,
            ));
        }
        Ok(StmtKind::Match { subject, arms })
    }

    fn parse_while(&mut self) -> Result<StmtKind, Diagnostic> {
        self.advance();
        let condition = self.parse_expr()?;
        let body = self.parse_block()?;
        Ok(StmtKind::While { condition, body })
    }

    fn parse_try(&mut self) -> Result<StmtKind, Diagnostic> {
        self.advance(); // try
        let body = self.parse_block()?;
        self.skip_newlines();
        self.expect(&TokenKind::Catch, "'catch' after try block")?;
        let (catch_var, _) = self.expect_ident("a name for the caught error")?;
        let catch_block = self.parse_block()?;
        Ok(StmtKind::Try {
            body,
            catch_var,
            catch_block,
        })
    }

    fn parse_for(&mut self) -> Result<StmtKind, Diagnostic> {
        self.advance();
        let (var, _) = self.expect_ident("a loop variable name")?;
        self.expect(&TokenKind::In, "'in'")?;
        let iter = self.parse_expr()?;
        let body = self.parse_block()?;
        Ok(StmtKind::For { var, iter, body })
    }

    fn parse_function(&mut self) -> Result<FunctionDecl, Diagnostic> {
        self.advance(); // fn
        let (name, _) = self.expect_ident("a function name")?;
        self.expect(&TokenKind::LParen, "'('")?;
        let mut params = Vec::new();
        if !self.check(&TokenKind::RParen) {
            loop {
                let (pname, _) = self.expect_ident("a parameter name")?;
                self.expect(&TokenKind::Colon, "':' after parameter name")?;
                let ty = self.parse_type()?;
                let default = if self.eat(&TokenKind::Assign) {
                    Some(self.parse_expr()?)
                } else {
                    None
                };
                params.push(Param {
                    name: pname,
                    ty,
                    default,
                });
                if !self.eat(&TokenKind::Comma) {
                    break;
                }
                if self.check(&TokenKind::RParen) {
                    break;
                }
            }
        }
        self.expect(&TokenKind::RParen, "')'")?;
        let return_type = if self.eat(&TokenKind::Arrow) {
            Some(self.parse_type()?)
        } else {
            None
        };
        let body = self.parse_block()?;
        Ok(FunctionDecl {
            name,
            params,
            return_type,
            body,
            public: false,
        })
    }

    fn parse_struct(&mut self) -> Result<StructDecl, Diagnostic> {
        self.advance(); // struct
        let (name, _) = self.expect_ident("a struct name")?;
        self.expect(&TokenKind::Colon, "':'")?;
        self.expect(&TokenKind::Newline, "a new line after ':'")?;
        self.expect(&TokenKind::Indent, "an indented block")?;
        let mut fields = Vec::new();
        self.skip_newlines();
        while !self.check(&TokenKind::Dedent) && !self.check(&TokenKind::Eof) {
            let (fname, span) = self.expect_ident("a field name")?;
            self.expect(&TokenKind::Colon, "':' after field name")?;
            let ty = self.parse_type()?;
            self.end_simple()?;
            fields.push(FieldDecl {
                name: fname,
                ty,
                span,
            });
            self.skip_newlines();
        }
        self.expect(&TokenKind::Dedent, "end of struct")?;
        Ok(StructDecl {
            name,
            fields,
            public: false,
        })
    }

    fn parse_enum(&mut self) -> Result<EnumDecl, Diagnostic> {
        self.advance(); // enum
        let (name, _) = self.expect_ident("an enum name")?;
        self.expect(&TokenKind::Colon, "':'")?;
        self.expect(&TokenKind::Newline, "a new line after ':'")?;
        self.expect(&TokenKind::Indent, "an indented block")?;
        let mut variants = Vec::new();
        self.skip_newlines();
        while !self.check(&TokenKind::Dedent) && !self.check(&TokenKind::Eof) {
            let (vname, span) = self.expect_ident("a variant name")?;
            self.end_simple()?;
            variants.push(EnumVariant { name: vname, span });
            self.skip_newlines();
        }
        let end_span = self.span();
        self.expect(&TokenKind::Dedent, "end of enum")?;
        if variants.is_empty() {
            return Err(Diagnostic::new(
                "an enum must have at least one variant",
                end_span,
            ));
        }
        Ok(EnumDecl {
            name,
            variants,
            public: false,
        })
    }

    // ---------- expressions (low to high precedence) ----------

    pub fn parse_expr(&mut self) -> Result<Expr, Diagnostic> {
        self.parse_or()
    }

    fn binary(left: Expr, op: BinaryOp, right: Expr) -> Expr {
        let span = left.span.clone();
        Expr {
            kind: ExprKind::Binary {
                op,
                left: Box::new(left),
                right: Box::new(right),
            },
            span,
        }
    }

    fn parse_or(&mut self) -> Result<Expr, Diagnostic> {
        let mut left = self.parse_and()?;
        while self.eat(&TokenKind::Or) {
            let right = self.parse_and()?;
            left = Self::binary(left, BinaryOp::Or, right);
        }
        Ok(left)
    }

    fn parse_and(&mut self) -> Result<Expr, Diagnostic> {
        let mut left = self.parse_not()?;
        while self.eat(&TokenKind::And) {
            let right = self.parse_not()?;
            left = Self::binary(left, BinaryOp::And, right);
        }
        Ok(left)
    }

    fn parse_not(&mut self) -> Result<Expr, Diagnostic> {
        if self.check(&TokenKind::Not) {
            let span = self.span();
            self.advance();
            let operand = self.parse_not()?;
            return Ok(Expr {
                kind: ExprKind::Unary {
                    op: UnaryOp::Not,
                    operand: Box::new(operand),
                },
                span,
            });
        }
        self.parse_comparison()
    }

    fn parse_comparison(&mut self) -> Result<Expr, Diagnostic> {
        let mut left = self.parse_range()?;
        loop {
            let op = match self.peek() {
                TokenKind::EqEq => BinaryOp::Eq,
                TokenKind::NotEq => BinaryOp::NotEq,
                TokenKind::Lt => BinaryOp::Lt,
                TokenKind::LtEq => BinaryOp::LtEq,
                TokenKind::Gt => BinaryOp::Gt,
                TokenKind::GtEq => BinaryOp::GtEq,
                _ => break,
            };
            self.advance();
            let right = self.parse_range()?;
            left = Self::binary(left, op, right);
        }
        Ok(left)
    }

    fn parse_range(&mut self) -> Result<Expr, Diagnostic> {
        let left = self.parse_additive()?;
        if self.eat(&TokenKind::DotDot) {
            let right = self.parse_additive()?;
            return Ok(Self::binary(left, BinaryOp::Range, right));
        }
        Ok(left)
    }

    fn parse_additive(&mut self) -> Result<Expr, Diagnostic> {
        let mut left = self.parse_multiplicative()?;
        loop {
            let op = match self.peek() {
                TokenKind::Plus => BinaryOp::Add,
                TokenKind::Minus => BinaryOp::Sub,
                _ => break,
            };
            self.advance();
            let right = self.parse_multiplicative()?;
            left = Self::binary(left, op, right);
        }
        Ok(left)
    }

    fn parse_multiplicative(&mut self) -> Result<Expr, Diagnostic> {
        let mut left = self.parse_unary()?;
        loop {
            let op = match self.peek() {
                TokenKind::Star => BinaryOp::Mul,
                TokenKind::Slash => BinaryOp::Div,
                TokenKind::Percent => BinaryOp::Mod,
                _ => break,
            };
            self.advance();
            let right = self.parse_unary()?;
            left = Self::binary(left, op, right);
        }
        Ok(left)
    }

    fn parse_unary(&mut self) -> Result<Expr, Diagnostic> {
        if self.check(&TokenKind::Minus) {
            let span = self.span();
            self.advance();
            let operand = self.parse_unary()?;
            return Ok(Expr {
                kind: ExprKind::Unary {
                    op: UnaryOp::Neg,
                    operand: Box::new(operand),
                },
                span,
            });
        }
        self.parse_postfix()
    }

    fn parse_postfix(&mut self) -> Result<Expr, Diagnostic> {
        let mut expr = self.parse_primary()?;
        loop {
            let span = expr.span.clone();
            if self.eat(&TokenKind::LParen) {
                let args = self.parse_arguments()?;
                expr = Expr {
                    kind: ExprKind::Call {
                        callee: Box::new(expr),
                        args,
                    },
                    span,
                };
            } else if self.eat(&TokenKind::Dot) {
                let (name, _) = self.expect_ident("a field or method name")?;
                expr = Expr {
                    kind: ExprKind::Field {
                        object: Box::new(expr),
                        name,
                    },
                    span,
                };
            } else if self.eat(&TokenKind::LBracket) {
                let index = self.parse_expr()?;
                self.expect(&TokenKind::RBracket, "']'")?;
                expr = Expr {
                    kind: ExprKind::Index {
                        object: Box::new(expr),
                        index: Box::new(index),
                    },
                    span,
                };
            } else {
                break;
            }
        }
        Ok(expr)
    }

    fn parse_arguments(&mut self) -> Result<Vec<Argument>, Diagnostic> {
        let mut args = Vec::new();
        if !self.check(&TokenKind::RParen) {
            loop {
                let mut name = None;
                if let TokenKind::Identifier(n) = self.peek().clone()
                    && *self.peek_at(1) == TokenKind::Assign
                {
                    self.advance();
                    self.advance();
                    name = Some(n);
                }
                let value = self.parse_expr()?;
                args.push(Argument { name, value });
                if !self.eat(&TokenKind::Comma) {
                    break;
                }
                if self.check(&TokenKind::RParen) {
                    break;
                }
            }
        }
        self.expect(&TokenKind::RParen, "')'")?;
        Ok(args)
    }

    fn parse_primary(&mut self) -> Result<Expr, Diagnostic> {
        let span = self.span();
        match self.peek().clone() {
            TokenKind::IntLiteral(v) => {
                self.advance();
                Ok(Expr {
                    kind: ExprKind::Int(v),
                    span,
                })
            }
            TokenKind::FloatLiteral(v) => {
                self.advance();
                Ok(Expr {
                    kind: ExprKind::Float(v),
                    span,
                })
            }
            TokenKind::StringLiteral(s) => {
                self.advance();
                Ok(Expr {
                    kind: ExprKind::Str(s),
                    span,
                })
            }
            TokenKind::True => {
                self.advance();
                Ok(Expr {
                    kind: ExprKind::Bool(true),
                    span,
                })
            }
            TokenKind::False => {
                self.advance();
                Ok(Expr {
                    kind: ExprKind::Bool(false),
                    span,
                })
            }
            TokenKind::None => {
                self.advance();
                Ok(Expr {
                    kind: ExprKind::NoneLit,
                    span,
                })
            }
            TokenKind::Identifier(name) => {
                self.advance();
                Ok(Expr {
                    kind: ExprKind::Ident(name),
                    span,
                })
            }
            TokenKind::LParen => {
                self.advance();
                let first = self.parse_expr()?;
                if self.check(&TokenKind::Comma) {
                    let mut items = vec![first];
                    while self.eat(&TokenKind::Comma) {
                        if self.check(&TokenKind::RParen) {
                            break;
                        }
                        items.push(self.parse_expr()?);
                    }
                    self.expect(&TokenKind::RParen, "')'")?;
                    if items.len() < 2 {
                        return Err(Diagnostic::new(
                            "a tuple needs at least 2 elements",
                            span.clone(),
                        ));
                    }
                    return Ok(Expr {
                        kind: ExprKind::Tuple(items),
                        span,
                    });
                }
                self.expect(&TokenKind::RParen, "')'")?;
                Ok(first)
            }
            TokenKind::LBracket => {
                self.advance();
                let mut items = Vec::new();
                if !self.check(&TokenKind::RBracket) {
                    loop {
                        items.push(self.parse_expr()?);
                        if !self.eat(&TokenKind::Comma) {
                            break;
                        }
                        if self.check(&TokenKind::RBracket) {
                            break;
                        }
                    }
                }
                self.expect(&TokenKind::RBracket, "']'")?;
                Ok(Expr {
                    kind: ExprKind::List(items),
                    span,
                })
            }
            other => Err(Diagnostic::new(
                format!("expected an expression, found {:?}", other),
                span,
            )),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::compiler::lexer::Lexer;

    fn parse_result(src: &str) -> Result<Program, Diagnostic> {
        let tokens = Lexer::new(src).tokenize().unwrap();
        Parser::new(tokens).parse_program()
    }

    fn parse(src: &str) -> Program {
        parse_result(src).unwrap()
    }

    #[test]
    fn import_simple() {
        let p = parse("import utils\n");
        assert_eq!(
            p.statements[0].kind,
            StmtKind::Import {
                module: "utils".to_string(),
                alias: None
            }
        );
    }

    #[test]
    fn import_with_alias() {
        let p = parse("import utils as u\n");
        assert_eq!(
            p.statements[0].kind,
            StmtKind::Import {
                module: "utils".to_string(),
                alias: Some("u".to_string())
            }
        );
    }

    #[test]
    fn from_import_names() {
        let p = parse("from utils import add, sub as minus\n");
        match &p.statements[0].kind {
            StmtKind::FromImport { module, names } => {
                assert_eq!(module, "utils");
                assert_eq!(names.len(), 2);
                assert_eq!(names[0].name, "add");
                assert_eq!(names[0].alias, None);
                assert_eq!(names[1].name, "sub");
                assert_eq!(names[1].alias, Some("minus".to_string()));
            }
            other => panic!("expected FromImport, got {:?}", other),
        }
    }

    #[test]
    fn import_missing_module_name_is_error() {
        assert!(parse_result("import\n").is_err());
    }

    #[test]
    fn from_import_missing_import_keyword_is_error() {
        assert!(parse_result("from utils add\n").is_err());
    }

    #[test]
    fn pub_fn_is_public() {
        let p = parse("pub fn add(a: Int, b: Int) -> Int:\n    return a + b\n");
        match &p.statements[0].kind {
            StmtKind::Function(f) => assert!(f.public),
            other => panic!("expected Function, got {:?}", other),
        }
    }

    #[test]
    fn plain_fn_is_not_public() {
        let p = parse("fn add(a: Int, b: Int) -> Int:\n    return a + b\n");
        match &p.statements[0].kind {
            StmtKind::Function(f) => assert!(!f.public),
            other => panic!("expected Function, got {:?}", other),
        }
    }

    #[test]
    fn pub_struct_is_public() {
        let p = parse("pub struct Point:\n    x: Int\n    y: Int\n");
        match &p.statements[0].kind {
            StmtKind::Struct(s) => assert!(s.public),
            other => panic!("expected Struct, got {:?}", other),
        }
    }

    #[test]
    fn pub_let_is_error() {
        assert!(parse_result("pub let x = 1\n").is_err());
    }
    #[test]
    fn let_statement() {
        let p = parse("let x: Int = 5\n");
        assert_eq!(p.statements.len(), 1);
        match &p.statements[0].kind {
            StmtKind::Let {
                name, mutable, ty, ..
            } => {
                assert_eq!(name, "x");
                assert!(!mutable);
                assert_eq!(ty.as_ref().unwrap().name, "Int");
            }
            other => panic!("unexpected {:?}", other),
        }
    }

    #[test]
    fn operator_precedence() {
        let p = parse("1 + 2 * 3\n");
        match &p.statements[0].kind {
            StmtKind::Expr(e) => match &e.kind {
                ExprKind::Binary { op, right, .. } => {
                    assert_eq!(*op, BinaryOp::Add);
                    assert!(matches!(
                        right.kind,
                        ExprKind::Binary {
                            op: BinaryOp::Mul,
                            ..
                        }
                    ));
                }
                other => panic!("unexpected {:?}", other),
            },
            other => panic!("unexpected {:?}", other),
        }
    }

    #[test]
    fn function_declaration() {
        let p = parse("fn add(a: Int, b: Int) -> Int:\n    return a + b\n");
        match &p.statements[0].kind {
            StmtKind::Function(f) => {
                assert_eq!(f.name, "add");
                assert_eq!(f.params.len(), 2);
                assert_eq!(f.return_type.as_ref().unwrap().name, "Int");
                assert_eq!(f.body.len(), 1);
            }
            other => panic!("unexpected {:?}", other),
        }
    }

    #[test]
    fn if_elif_else() {
        let p = parse("if a:\n    x = 1\nelif b:\n    x = 2\nelse:\n    x = 3\n");
        match &p.statements[0].kind {
            StmtKind::If { else_block, .. } => {
                let eb = else_block.as_ref().unwrap();
                assert_eq!(eb.len(), 1);
                assert!(matches!(eb[0].kind, StmtKind::If { .. }));
            }
            other => panic!("unexpected {:?}", other),
        }
    }

    #[test]
    fn for_range_loop() {
        let p = parse("for i in 0..10:\n    print(i)\n");
        match &p.statements[0].kind {
            StmtKind::For { var, body, .. } => {
                assert_eq!(var, "i");
                assert_eq!(body.len(), 1);
            }
            other => panic!("unexpected {:?}", other),
        }
    }

    #[test]
    fn struct_and_named_arguments() {
        let p = parse(
            "struct User:\n    name: String\n    age: Int\n\nlet user = User(name = \"A\", age = 20)\n",
        );
        assert_eq!(p.statements.len(), 2);
        match &p.statements[1].kind {
            StmtKind::Let { value, .. } => match &value.kind {
                ExprKind::Call { args, .. } => {
                    assert_eq!(args.len(), 2);
                    assert_eq!(args[0].name, Some("name".to_string()));
                }
                other => panic!("unexpected {:?}", other),
            },
            other => panic!("unexpected {:?}", other),
        }
    }

    #[test]
    fn multiline_call() {
        let p = parse("let u = User(\n    name = \"a\",\n    age = 1\n)\n");
        assert_eq!(p.statements.len(), 1);
    }

    #[test]
    fn error_missing_variable_name() {
        assert!(parse_result("let = 5\n").is_err());
    }

    #[test]
    fn error_missing_colon() {
        assert!(parse_result("if x\n    y\n").is_err());
    }

    #[test]
    fn error_parameter_without_type() {
        assert!(parse_result("fn f(a):\n    return 1\n").is_err());
    }

    #[test]
    fn try_catch_statement() {
        let p = parse("try:\n    print(1)\ncatch e:\n    print(e)\n");
        match &p.statements[0].kind {
            StmtKind::Try {
                body,
                catch_var,
                catch_block,
            } => {
                assert_eq!(body.len(), 1);
                assert_eq!(catch_var, "e");
                assert_eq!(catch_block.len(), 1);
            }
            other => panic!("expected try, got {:?}", other),
        }
    }

    #[test]
    fn throw_statement() {
        let p = parse("throw \"boom\"\n");
        assert!(matches!(p.statements[0].kind, StmtKind::Throw(_)));
    }

    #[test]
    fn error_try_without_catch() {
        assert!(parse_result("try:\n    print(1)\n").is_err());
    }
}

#[cfg(test)]
mod enum_parse_tests {
    use super::*;
    use crate::compiler::lexer::Lexer;

    fn parse_result(src: &str) -> Result<Program, Diagnostic> {
        let tokens = Lexer::new(src).tokenize().unwrap();
        Parser::new(tokens).parse_program()
    }

    #[test]
    fn enum_with_three_variants() {
        let p = parse_result("enum Color:\n    Red\n    Green\n    Blue\n").unwrap();
        match &p.statements[0].kind {
            StmtKind::Enum(e) => {
                assert_eq!(e.name, "Color");
                assert!(!e.public);
                let names: Vec<&str> = e.variants.iter().map(|v| v.name.as_str()).collect();
                assert_eq!(names, vec!["Red", "Green", "Blue"]);
            }
            other => panic!("expected Enum, got {:?}", other),
        }
    }

    #[test]
    fn pub_enum() {
        let p = parse_result("pub enum Mode:\n    On\n    Off\n").unwrap();
        match &p.statements[0].kind {
            StmtKind::Enum(e) => assert!(e.public),
            other => panic!("expected Enum, got {:?}", other),
        }
    }

    #[test]
    fn enum_needs_a_name() {
        assert!(parse_result("enum:\n    Red\n").is_err());
    }

    #[test]
    fn enum_needs_a_variant_name() {
        assert!(parse_result("enum Color:\n    1\n").is_err());
    }
}

#[cfg(test)]
mod match_parse_tests {
    use super::*;
    use crate::compiler::lexer::Lexer;

    fn parse_result(src: &str) -> Result<Program, Diagnostic> {
        let tokens = Lexer::new(src).tokenize().unwrap();
        Parser::new(tokens).parse_program()
    }

    #[test]
    fn match_with_variants_and_wildcard() {
        let src = "match c:\n    Color.Red:\n        print(1)\n    Color.Green:\n        print(2)\n    _:\n        print(3)\n";
        let p = parse_result(src).unwrap();
        match &p.statements[0].kind {
            StmtKind::Match { arms, .. } => {
                assert_eq!(arms.len(), 3);
                assert!(matches!(arms[0].pattern, Pattern::Value(_)));
                assert!(matches!(arms[2].pattern, Pattern::Wildcard));
                assert_eq!(arms[1].body.len(), 1);
            }
            other => panic!("expected Match, got {:?}", other),
        }
    }

    #[test]
    fn match_on_int_literals() {
        let p =
            parse_result("match n:\n    0:\n        print(0)\n    1:\n        print(1)\n").unwrap();
        match &p.statements[0].kind {
            StmtKind::Match { arms, .. } => assert_eq!(arms.len(), 2),
            other => panic!("expected Match, got {:?}", other),
        }
    }

    #[test]
    fn match_needs_an_arm() {
        assert!(parse_result("match n:\n").is_err());
    }

    #[test]
    fn match_arm_needs_a_colon() {
        assert!(parse_result("match n:\n    0\n").is_err());
    }
}

#[cfg(test)]
mod const_parse_tests {
    use super::*;
    use crate::compiler::lexer::Lexer;

    fn parse_result(src: &str) -> Result<Program, Diagnostic> {
        let tokens = Lexer::new(src).tokenize().unwrap();
        Parser::new(tokens).parse_program()
    }

    #[test]
    fn const_with_type() {
        let p = parse_result("const MAX: Int = 100\n").unwrap();
        match &p.statements[0].kind {
            StmtKind::Let {
                name,
                mutable,
                constant,
                ty,
                ..
            } => {
                assert_eq!(name, "MAX");
                assert!(!*mutable);
                assert!(*constant);
                assert!(ty.is_some());
            }
            other => panic!("expected Let, got {:?}", other),
        }
    }

    #[test]
    fn const_without_type_and_let_is_not_const() {
        let p = parse_result("const A = 1\nlet b = 2\n").unwrap();
        assert!(matches!(
            &p.statements[0].kind,
            StmtKind::Let { constant: true, .. }
        ));
        assert!(matches!(
            &p.statements[1].kind,
            StmtKind::Let {
                constant: false,
                ..
            }
        ));
    }

    #[test]
    fn const_needs_a_value() {
        assert!(parse_result("const A: Int\n").is_err());
    }
}

#[cfg(test)]
mod tuple_parse_tests {
    use super::*;
    use crate::compiler::lexer::Lexer;

    fn parse_result(src: &str) -> Result<Program, Diagnostic> {
        let tokens = Lexer::new(src).tokenize().unwrap();
        Parser::new(tokens).parse_program()
    }

    fn let_value(src: &str) -> Expr {
        let p = parse_result(src).unwrap();
        match &p.statements[0].kind {
            StmtKind::Let { value, .. } => value.clone(),
            other => panic!("expected Let, got {:?}", other),
        }
    }

    #[test]
    fn two_element_tuple() {
        match let_value("let p = (1, \"a\")\n").kind {
            ExprKind::Tuple(items) => assert_eq!(items.len(), 2),
            other => panic!("expected Tuple, got {:?}", other),
        }
    }

    #[test]
    fn three_elements_with_trailing_comma() {
        match let_value("let p = (1, 2, 3,)\n").kind {
            ExprKind::Tuple(items) => assert_eq!(items.len(), 3),
            other => panic!("expected Tuple, got {:?}", other),
        }
    }

    #[test]
    fn parentheses_without_comma_are_grouping() {
        assert!(matches!(let_value("let a = (1)\n").kind, ExprKind::Int(1)));
        assert!(matches!(
            let_value("let a = (1 + 2)\n").kind,
            ExprKind::Binary { .. }
        ));
    }

    #[test]
    fn one_element_tuple_is_error() {
        assert!(parse_result("let a = (1,)\n").is_err());
    }
}

#[cfg(test)]
mod let_tuple_parse_tests {
    use super::*;
    use crate::compiler::lexer::Lexer;

    fn parse_result(src: &str) -> Result<Program, Diagnostic> {
        let tokens = Lexer::new(src).tokenize().unwrap();
        Parser::new(tokens).parse_program()
    }

    #[test]
    fn let_tuple_two_names() {
        let p = parse_result("let (a, b) = (1, 2)\n").unwrap();
        match &p.statements[0].kind {
            StmtKind::LetTuple { names, mutable, .. } => {
                assert_eq!(names, &vec!["a".to_string(), "b".to_string()]);
                assert!(!*mutable);
            }
            other => panic!("expected LetTuple, got {:?}", other),
        }
    }

    #[test]
    fn mut_tuple_three_names() {
        let p = parse_result("mut (a, b, c) = (1, 2, 3)\n").unwrap();
        assert!(matches!(
            &p.statements[0].kind,
            StmtKind::LetTuple { mutable: true, .. }
        ));
    }

    #[test]
    fn one_name_is_error() {
        assert!(parse_result("let (a) = (1, 2)\n").is_err());
    }

    #[test]
    fn plain_let_still_works() {
        let p = parse_result("let a = 1\n").unwrap();
        assert!(matches!(&p.statements[0].kind, StmtKind::Let { .. }));
    }
}

#[cfg(test)]
mod tuple_pattern_error_tests {
    use super::*;
    use crate::compiler::lexer::Lexer;

    fn first_message(src: &str) -> String {
        let tokens = Lexer::new(src).tokenize().unwrap();
        Parser::new(tokens).parse_program().unwrap_err().message
    }

    #[test]
    fn nested_tuple_pattern_is_error() {
        assert_eq!(
            first_message("let ((a, b), c) = (1, 2)\n"),
            "expected a variable name, found '(' (tuple patterns are not supported here)"
        );
    }

    #[test]
    fn tuple_parameter_is_error() {
        assert_eq!(
            first_message("fn f((a, b): Int) -> Int:\n    return 1\n"),
            "expected a parameter name, found '(' (tuple patterns are not supported here)"
        );
    }
}
