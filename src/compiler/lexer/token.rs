#[derive(Debug, Clone, PartialEq)]
pub struct Span {
    pub line: usize,
    pub column: usize,
}

#[derive(Debug, Clone, PartialEq)]
pub enum TokenKind {
    // Keywords
    Let, Mut, Fn, Return, If, Else, Elif,
    For, While, In, Break, Continue,
    Struct, Enum, Type, Import, From, As,
    Pub, Private, Try, Catch, Throw,
    And, Or, Not, Const, True, False, None, Match,

    // Literals
    Identifier(String),
    IntLiteral(i64),
    FloatLiteral(f64),
    StringLiteral(String),

    // Operators and symbols
    Plus, Minus, Star, Slash, Percent,
    Assign, EqEq, NotEq, Lt, LtEq, Gt, GtEq,
    Arrow, Colon, Comma, Dot, DotDot,
    LParen, RParen, LBracket, RBracket, LBrace, RBrace,

    // Structure
    Newline, Indent, Dedent, Eof,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Token {
    pub kind: TokenKind,
    pub span: Span,
}

pub fn keyword_from_str(s: &str) -> Option<TokenKind> {
    Some(match s {
        "let" => TokenKind::Let,
        "mut" => TokenKind::Mut,
        "fn" => TokenKind::Fn,
        "return" => TokenKind::Return,
        "if" => TokenKind::If,
        "else" => TokenKind::Else,
        "elif" => TokenKind::Elif,
        "for" => TokenKind::For,
        "while" => TokenKind::While,
        "in" => TokenKind::In,
        "break" => TokenKind::Break,
        "continue" => TokenKind::Continue,
        "struct" => TokenKind::Struct,
        "enum" => TokenKind::Enum,
        "type" => TokenKind::Type,
        "import" => TokenKind::Import,
        "from" => TokenKind::From,
        "as" => TokenKind::As,
        "pub" => TokenKind::Pub,
        "private" => TokenKind::Private,
        "try" => TokenKind::Try,
        "catch" => TokenKind::Catch,
        "throw" => TokenKind::Throw,
        "and" => TokenKind::And,
        "or" => TokenKind::Or,
        "not" => TokenKind::Not,
        "const" => TokenKind::Const,
        "true" => TokenKind::True,
        "false" => TokenKind::False,
        "none" => TokenKind::None,
        "match" => TokenKind::Match,
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keyword_lookup_works() {
        assert_eq!(keyword_from_str("let"), Some(TokenKind::Let));
        assert_eq!(keyword_from_str("fn"), Some(TokenKind::Fn));
        assert_eq!(keyword_from_str("hello"), None);
    }
}
