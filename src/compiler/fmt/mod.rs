//! Token-based source formatter (dev tool #3).
//!
//! Strategy: re-emit the token stream with canonical whitespace. Comments
//! are preserved verbatim. Idempotent by design (format(format(x)) ==
//! format(x)) — this is verified in `tests/fmt.rs`.
//!
//! Canonical rules:
//! - 4-space indent per `Indent`, dedent per `Dedent`.
//! - One statement per line (`Newline` emits `\n`).
//! - Binary operators get spaces on both sides.
//! - `:` in type annotations gets a space after.
//! - `,` gets a space after (but not before a closing bracket/brace).
//! - Opening brackets emit no space before the next token.
//! - Comments are emitted verbatim (leading `#` already included).

use crate::compiler::lexer::token::{Span, Token, TokenKind};

pub fn format_tokens(tokens: &[Token]) -> Result<String, (String, Span)> {
    let mut out = String::new();
    let mut indent: usize = 0;
    let mut prev: Option<&TokenKind> = None;
    let mut at_line_start = true;

    for t in tokens {
        match &t.kind {
            TokenKind::Eof => break,
            TokenKind::Indent => {
                indent += 1;
                continue;
            }
            TokenKind::Dedent => {
                indent = indent.saturating_sub(1);
                continue;
            }
            TokenKind::Newline => {
                out.push('\n');
                at_line_start = true;
                prev = Some(&t.kind);
                continue;
            }
            _ => {}
        }

        if at_line_start {
            for _ in 0..indent {
                out.push_str("    ");
            }
            at_line_start = false;
        } else if needs_space_before(prev, &t.kind) {
            out.push(' ');
        }

        out.push_str(&render_token(&t.kind));

        // Some tokens pull a space after them, handled by needs_space_before
        // on the next iteration. We just record the previous kind here.
        prev = Some(&t.kind);
    }

    // Final newline if the source didn't end with one (EOF without Newline).
    if !out.is_empty() && !out.ends_with('\n') {
        out.push('\n');
    }
    Ok(out)
}

fn needs_space_before(prev: Option<&TokenKind>, cur: &TokenKind) -> bool {
    let prev = match prev {
        Some(p) => p,
        None => return false,
    };

    // No space after an opening bracket.
    if matches!(
        prev,
        TokenKind::LParen | TokenKind::LBracket | TokenKind::LBrace
    ) {
        return false;
    }
    // No space before a closing bracket, comma, dot, or double-dot.
    if matches!(
        cur,
        TokenKind::RParen
            | TokenKind::RBracket
            | TokenKind::RBrace
            | TokenKind::Comma
            | TokenKind::Dot
            | TokenKind::DotDot
    ) {
        return false;
    }
    // No space before `:` (type annotation attaches to the name).
    if matches!(cur, TokenKind::Colon) {
        return false;
    }
    // No space after `:` in type annotations (yes space after? we emit space
    // before the *next* token — so return false here means no space after
    // colon; we want a space after the colon in `name: Type`).
    // Handled below as "space after colon".
    // No space before `(` when it's a call: `f(x)`. We can't tell calls from
    // grouping without AST. Use the heuristic: no space before `(` if prev
    // is an identifier.
    if matches!(cur, TokenKind::LParen) && matches!(prev, TokenKind::Identifier(_)) {
        return false;
    }
    // Default: a space between tokens.
    true
}

fn render_token(kind: &TokenKind) -> String {
    match kind {
        TokenKind::Let => "let".into(),
        TokenKind::Mut => "mut".into(),
        TokenKind::Fn => "fn".into(),
        TokenKind::Return => "return".into(),
        TokenKind::If => "if".into(),
        TokenKind::Else => "else".into(),
        TokenKind::Elif => "elif".into(),
        TokenKind::For => "for".into(),
        TokenKind::While => "while".into(),
        TokenKind::In => "in".into(),
        TokenKind::Break => "break".into(),
        TokenKind::Continue => "continue".into(),
        TokenKind::Struct => "struct".into(),
        TokenKind::Enum => "enum".into(),
        TokenKind::Type => "type".into(),
        TokenKind::Import => "import".into(),
        TokenKind::From => "from".into(),
        TokenKind::As => "as".into(),
        TokenKind::Pub => "pub".into(),
        TokenKind::Private => "private".into(),
        TokenKind::Try => "try".into(),
        TokenKind::Catch => "catch".into(),
        TokenKind::Throw => "throw".into(),
        TokenKind::And => "and".into(),
        TokenKind::Or => "or".into(),
        TokenKind::Not => "not".into(),
        TokenKind::Const => "const".into(),
        TokenKind::True => "true".into(),
        TokenKind::False => "false".into(),
        TokenKind::None => "none".into(),
        TokenKind::Match => "match".into(),
        TokenKind::Identifier(s) => s.clone(),
        TokenKind::IntLiteral(i) => i.to_string(),
        TokenKind::IntLiteralMin => "-9223372036854775808".into(),
        TokenKind::FloatLiteral(f) => {
            // Shortest representation that parses back to the same f64.
            let s = format!("{:?}", f);
            s
        }
        TokenKind::StringLiteral(s) => {
            let mut out = String::with_capacity(s.len() + 2);
            out.push('"');
            for c in s.chars() {
                match c {
                    '"' => out.push_str("\\\""),
                    '\\' => out.push_str("\\\\"),
                    '\n' => out.push_str("\\n"),
                    '\t' => out.push_str("\\t"),
                    '\r' => out.push_str("\\r"),
                    _ => out.push(c),
                }
            }
            out.push('"');
            out
        }
        TokenKind::Plus => "+".into(),
        TokenKind::Minus => "-".into(),
        TokenKind::Star => "*".into(),
        TokenKind::Slash => "/".into(),
        TokenKind::Percent => "%".into(),
        TokenKind::Assign => "=".into(),
        TokenKind::EqEq => "==".into(),
        TokenKind::NotEq => "!=".into(),
        TokenKind::Lt => "<".into(),
        TokenKind::LtEq => "<=".into(),
        TokenKind::Gt => ">".into(),
        TokenKind::GtEq => ">=".into(),
        TokenKind::Arrow => "->".into(),
        TokenKind::Colon => ":".into(),
        TokenKind::Comma => ",".into(),
        TokenKind::Dot => ".".into(),
        TokenKind::DotDot => "..".into(),
        TokenKind::LParen => "(".into(),
        TokenKind::RParen => ")".into(),
        TokenKind::LBracket => "[".into(),
        TokenKind::RBracket => "]".into(),
        TokenKind::LBrace => "{".into(),
        TokenKind::RBrace => "}".into(),
        TokenKind::Comment(s) => s.clone(),
        TokenKind::Newline | TokenKind::Indent | TokenKind::Dedent | TokenKind::Eof => {
            String::new()
        }
    }
}
