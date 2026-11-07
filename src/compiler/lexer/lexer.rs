use super::token::{Span, Token, TokenKind, keyword_from_str};
use crate::compiler::diagnostics::Diagnostic;

pub struct Lexer {
    chars: Vec<char>,
    pos: usize,
    line: usize,
    column: usize,
    indent_stack: Vec<usize>,
    tokens: Vec<Token>,
}

impl Lexer {
    pub fn new(source: &str) -> Self {
        Lexer {
            chars: source.replace("\r\n", "\n").chars().collect(),
            pos: 0,
            line: 1,
            column: 1,
            indent_stack: vec![0],
            tokens: Vec::new(),
        }
    }

    fn peek(&self) -> Option<char> {
        self.chars.get(self.pos).copied()
    }

    fn peek_next(&self) -> Option<char> {
        self.chars.get(self.pos + 1).copied()
    }

    fn advance(&mut self) -> Option<char> {
        let c = self.chars.get(self.pos).copied()?;
        self.pos += 1;
        if c == '\n' {
            self.line += 1;
            self.column = 1;
        } else {
            self.column += 1;
        }
        Some(c)
    }

    fn span(&self) -> Span {
        Span {
            line: self.line,
            column: self.column,
        }
    }

    fn match_next(&mut self, expected: char) -> bool {
        if self.peek() == Some(expected) {
            self.advance();
            true
        } else {
            false
        }
    }

    fn single(&mut self, kind: TokenKind, span: Span) {
        self.advance();
        self.tokens.push(Token { kind, span });
    }

    fn skip_comment(&mut self) {
        while let Some(c) = self.peek() {
            if c == '\n' {
                break;
            }
            self.advance();
        }
    }

    fn handle_indent(&mut self, indent: usize) -> Result<(), Diagnostic> {
        let span = self.span();
        let current = *self.indent_stack.last().unwrap();
        if indent > current {
            self.indent_stack.push(indent);
            self.tokens.push(Token {
                kind: TokenKind::Indent,
                span,
            });
        } else if indent < current {
            while *self.indent_stack.last().unwrap() > indent {
                self.indent_stack.pop();
                self.tokens.push(Token {
                    kind: TokenKind::Dedent,
                    span: span.clone(),
                });
            }
            if *self.indent_stack.last().unwrap() != indent {
                return Err(Diagnostic::new("inconsistent indentation", span));
            }
        }
        Ok(())
    }

    pub fn tokenize(mut self) -> Result<Vec<Token>, Diagnostic> {
        let mut at_line_start = true;
        let mut depth: usize = 0;

        loop {
            if at_line_start && depth == 0 {
                let mut indent = 0;
                while self.peek() == Some(' ') {
                    self.advance();
                    indent += 1;
                }
                if self.peek() == Some('\t') {
                    return Err(Diagnostic::new(
                        "tabs are not allowed; use 4 spaces",
                        self.span(),
                    ));
                }
                match self.peek() {
                    None => break,
                    Some('\n') => {
                        self.advance();
                        continue;
                    }
                    Some('#') => {
                        self.skip_comment();
                        continue;
                    }
                    _ => {}
                }
                self.handle_indent(indent)?;
                at_line_start = false;
            }

            let c = match self.peek() {
                Some(c) => c,
                None => break,
            };
            let span = self.span();

            match c {
                ' ' | '\t' | '\r' => {
                    self.advance();
                }
                '#' => self.skip_comment(),
                '\n' => {
                    self.advance();
                    if depth == 0 {
                        self.tokens.push(Token {
                            kind: TokenKind::Newline,
                            span,
                        });
                        at_line_start = true;
                    }
                }
                '"' => {
                    self.advance();
                    let mut s = String::new();
                    loop {
                        match self.advance() {
                            None | Some('\n') => {
                                return Err(Diagnostic::new("unterminated string", span));
                            }
                            Some('"') => break,
                            Some('\\') => match self.advance() {
                                Some('n') => s.push('\n'),
                                Some('t') => s.push('\t'),
                                Some('"') => s.push('"'),
                                Some('\\') => s.push('\\'),
                                _ => {
                                    return Err(Diagnostic::new(
                                        "invalid escape sequence",
                                        self.span(),
                                    ));
                                }
                            },
                            Some(ch) => s.push(ch),
                        }
                    }
                    self.tokens.push(Token {
                        kind: TokenKind::StringLiteral(s),
                        span,
                    });
                }
                c if c.is_ascii_digit() => {
                    let mut s = String::new();
                    while let Some(d) = self.peek() {
                        if d.is_ascii_digit() {
                            s.push(d);
                            self.advance();
                        } else if d == '_' {
                            self.advance();
                        } else {
                            break;
                        }
                    }
                    let mut is_float = false;
                    if self.peek() == Some('.')
                        && self.peek_next().map_or(false, |n| n.is_ascii_digit())
                    {
                        is_float = true;
                        s.push('.');
                        self.advance();
                        while let Some(d) = self.peek() {
                            if d.is_ascii_digit() {
                                s.push(d);
                                self.advance();
                            } else {
                                break;
                            }
                        }
                    }
                    let kind = if is_float {
                        match s.parse::<f64>() {
                            Ok(v) => TokenKind::FloatLiteral(v),
                            Err(_) => return Err(Diagnostic::new("invalid float literal", span)),
                        }
                    } else {
                        match s.parse::<i64>() {
                            Ok(v) => TokenKind::IntLiteral(v),
                            Err(_) => {
                                return Err(Diagnostic::new("integer literal too large", span));
                            }
                        }
                    };
                    self.tokens.push(Token { kind, span });
                }
                c if c.is_alphabetic() || c == '_' => {
                    let mut s = String::new();
                    while let Some(ch) = self.peek() {
                        if ch.is_alphanumeric() || ch == '_' {
                            s.push(ch);
                            self.advance();
                        } else {
                            break;
                        }
                    }
                    let kind = keyword_from_str(&s).unwrap_or(TokenKind::Identifier(s));
                    self.tokens.push(Token { kind, span });
                }
                '+' => self.single(TokenKind::Plus, span),
                '*' => self.single(TokenKind::Star, span),
                '/' => self.single(TokenKind::Slash, span),
                '%' => self.single(TokenKind::Percent, span),
                ':' => self.single(TokenKind::Colon, span),
                ',' => self.single(TokenKind::Comma, span),
                '-' => {
                    self.advance();
                    let kind = if self.match_next('>') {
                        TokenKind::Arrow
                    } else {
                        TokenKind::Minus
                    };
                    self.tokens.push(Token { kind, span });
                }
                '=' => {
                    self.advance();
                    let kind = if self.match_next('=') {
                        TokenKind::EqEq
                    } else {
                        TokenKind::Assign
                    };
                    self.tokens.push(Token { kind, span });
                }
                '<' => {
                    self.advance();
                    let kind = if self.match_next('=') {
                        TokenKind::LtEq
                    } else {
                        TokenKind::Lt
                    };
                    self.tokens.push(Token { kind, span });
                }
                '>' => {
                    self.advance();
                    let kind = if self.match_next('=') {
                        TokenKind::GtEq
                    } else {
                        TokenKind::Gt
                    };
                    self.tokens.push(Token { kind, span });
                }
                '.' => {
                    self.advance();
                    let kind = if self.match_next('.') {
                        TokenKind::DotDot
                    } else {
                        TokenKind::Dot
                    };
                    self.tokens.push(Token { kind, span });
                }
                '!' => {
                    self.advance();
                    if self.match_next('=') {
                        self.tokens.push(Token {
                            kind: TokenKind::NotEq,
                            span,
                        });
                    } else {
                        return Err(Diagnostic::new("unexpected '!'; use 'not' instead", span));
                    }
                }
                '(' => {
                    depth += 1;
                    self.single(TokenKind::LParen, span);
                }
                ')' => {
                    depth = depth.saturating_sub(1);
                    self.single(TokenKind::RParen, span);
                }
                '[' => {
                    depth += 1;
                    self.single(TokenKind::LBracket, span);
                }
                ']' => {
                    depth = depth.saturating_sub(1);
                    self.single(TokenKind::RBracket, span);
                }
                '{' => {
                    depth += 1;
                    self.single(TokenKind::LBrace, span);
                }
                '}' => {
                    depth = depth.saturating_sub(1);
                    self.single(TokenKind::RBrace, span);
                }
                other => {
                    return Err(Diagnostic::new(
                        format!("unexpected character '{}'", other),
                        span,
                    ));
                }
            }
        }

        if depth > 0 {
            return Err(Diagnostic::new("unclosed bracket", self.span()));
        }

        if let Some(last) = self.tokens.last() {
            if last.kind != TokenKind::Newline {
                let span = self.span();
                self.tokens.push(Token {
                    kind: TokenKind::Newline,
                    span,
                });
            }
        }
        while self.indent_stack.len() > 1 {
            self.indent_stack.pop();
            let span = self.span();
            self.tokens.push(Token {
                kind: TokenKind::Dedent,
                span,
            });
        }
        let span = self.span();
        self.tokens.push(Token {
            kind: TokenKind::Eof,
            span,
        });
        Ok(self.tokens)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use TokenKind::*;

    fn kinds(src: &str) -> Vec<TokenKind> {
        Lexer::new(src)
            .tokenize()
            .unwrap()
            .into_iter()
            .map(|t| t.kind)
            .collect()
    }

    #[test]
    fn let_statement() {
        assert_eq!(
            kinds("let x = 5\n"),
            vec![
                Let,
                Identifier("x".into()),
                Assign,
                IntLiteral(5),
                Newline,
                Eof
            ]
        );
    }

    #[test]
    fn range_is_not_float() {
        assert_eq!(
            kinds("0..10"),
            vec![IntLiteral(0), DotDot, IntLiteral(10), Newline, Eof]
        );
    }

    #[test]
    fn float_literal() {
        assert_eq!(kinds("20.5"), vec![FloatLiteral(20.5), Newline, Eof]);
    }

    #[test]
    fn arrow_and_comparison() {
        assert_eq!(kinds("-> >= =="), vec![Arrow, GtEq, EqEq, Newline, Eof]);
    }

    #[test]
    fn indentation_blocks() {
        assert_eq!(
            kinds("if x:\n    y\n"),
            vec![
                If,
                Identifier("x".into()),
                Colon,
                Newline,
                Indent,
                Identifier("y".into()),
                Newline,
                Dedent,
                Eof
            ]
        );
    }

    #[test]
    fn comments_are_skipped() {
        assert_eq!(
            kinds("# hello\nlet a = 1 # note\n"),
            vec![
                Let,
                Identifier("a".into()),
                Assign,
                IntLiteral(1),
                Newline,
                Eof
            ]
        );
    }

    #[test]
    fn string_literal() {
        assert_eq!(
            kinds("\"RCA\""),
            vec![StringLiteral("RCA".into()), Newline, Eof]
        );
    }

    #[test]
    fn unterminated_string_is_error() {
        assert!(Lexer::new("\"abc").tokenize().is_err());
    }

    #[test]
    fn bad_indentation_is_error() {
        assert!(Lexer::new("if x:\n        a\n    b\n").tokenize().is_err());
    }
}
