//! Minimal JSON parser and stringifier implemented in pure Rust.
use crate::runtime::values::Value;

pub fn parse(src: &str) -> Result<Value, String> {
    let mut p = Parser {
        chars: src.chars().peekable(),
    };
    p.skip_ws();
    let v = p.parse_value()?;
    p.skip_ws();
    if p.chars.next().is_some() {
        return Err("trailing characters after JSON value".into());
    }
    Ok(v)
}

pub fn stringify(v: &Value) -> Result<String, String> {
    let mut out = String::new();
    write_value(v, &mut out)?;
    Ok(out)
}

fn write_value(v: &Value, out: &mut String) -> Result<(), String> {
    match v {
        Value::None => out.push_str("null"),
        Value::Bool(b) => out.push_str(if *b { "true" } else { "false" }),
        Value::Int(n) => out.push_str(&n.to_string()),
        Value::Float(x) => {
            if x.is_finite() {
                out.push_str(&x.to_string());
            } else {
                return Err("cannot stringify non-finite float".into());
            }
        }
        Value::Str(s) => {
            out.push('"');
            for c in s.chars() {
                match c {
                    '"' => out.push_str("\\\""),
                    '\\' => out.push_str("\\\\"),
                    '\n' => out.push_str("\\n"),
                    '\r' => out.push_str("\\r"),
                    '\t' => out.push_str("\\t"),
                    _ => out.push(c),
                }
            }
            out.push('"');
        }
        Value::List(items) => {
            out.push('[');
            for (i, item) in items.iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                write_value(item, out)?;
            }
            out.push(']');
        }
        Value::Map(pairs) => {
            out.push('{');
            for (i, (k, v)) in pairs.iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                let key_str = match k {
                    Value::Str(s) => s.clone(),
                    _ => return Err("JSON object keys must be strings".into()),
                };
                write_value(&Value::Str(key_str), out)?;
                out.push(':');
                write_value(v, out)?;
            }
            out.push('}');
        }
        _ => return Err("type not supported by JSON stringify".into()),
    }
    Ok(())
}

struct Parser<'a> {
    chars: std::iter::Peekable<std::str::Chars<'a>>,
}

impl<'a> Parser<'a> {
    fn skip_ws(&mut self) {
        while let Some(&c) = self.chars.peek() {
            if c.is_whitespace() {
                self.chars.next();
            } else {
                break;
            }
        }
    }
    fn parse_value(&mut self) -> Result<Value, String> {
        self.skip_ws();
        match self.chars.peek() {
            Some('{') => self.parse_object(),
            Some('[') => self.parse_array(),
            Some('"') => self.parse_string().map(Value::Str),
            Some('t') => self.parse_lit("true", Value::Bool(true)),
            Some('f') => self.parse_lit("false", Value::Bool(false)),
            Some('n') => self.parse_lit("null", Value::None),
            Some(c) if *c == '-' || c.is_ascii_digit() => self.parse_number(),
            Some(c) => Err(format!("unexpected character '{}'", c)),
            None => Err("unexpected end".into()),
        }
    }
    fn parse_lit(&mut self, lit: &str, val: Value) -> Result<Value, String> {
        for expected in lit.chars() {
            if self.chars.next() != Some(expected) {
                return Err("invalid literal".into());
            }
        }
        Ok(val)
    }
    fn parse_number(&mut self) -> Result<Value, String> {
        let mut s = String::new();
        if let Some('-') = self.chars.peek() {
            s.push(self.chars.next().unwrap());
        }
        while let Some(&c) = self.chars.peek() {
            if c.is_ascii_digit() {
                s.push(self.chars.next().unwrap());
            } else {
                break;
            }
        }
        if let Some('.') = self.chars.peek() {
            s.push(self.chars.next().unwrap());
            while let Some(&c) = self.chars.peek() {
                if c.is_ascii_digit() {
                    s.push(self.chars.next().unwrap());
                } else {
                    break;
                }
            }
            return Ok(Value::Float(s.parse().map_err(|_| "invalid float")?));
        }
        Ok(Value::Int(s.parse().map_err(|_| "invalid int")?))
    }
    fn parse_string(&mut self) -> Result<String, String> {
        self.chars.next();
        let mut s = String::new();
        loop {
            match self.chars.next() {
                Some('"') => break,
                Some('\\') => match self.chars.next() {
                    Some('"') => s.push('"'),
                    Some('\\') => s.push('\\'),
                    Some('/') => s.push('/'),
                    Some('n') => s.push('\n'),
                    Some('t') => s.push('\t'),
                    Some('r') => s.push('\r'),
                    _ => return Err("invalid escape".into()),
                },
                Some(c) => s.push(c),
                None => return Err("unterminated string".into()),
            }
        }
        Ok(s)
    }
    fn parse_array(&mut self) -> Result<Value, String> {
        self.chars.next();
        let mut items = Vec::new();
        self.skip_ws();
        if let Some(']') = self.chars.peek() {
            self.chars.next();
            return Ok(Value::List(items));
        }
        loop {
            items.push(self.parse_value()?);
            self.skip_ws();
            match self.chars.next() {
                Some(',') => {
                    self.skip_ws();
                }
                Some(']') => break,
                _ => return Err("expected ',' or ']'".into()),
            }
        }
        Ok(Value::List(items))
    }
    fn parse_object(&mut self) -> Result<Value, String> {
        self.chars.next();
        let mut pairs = Vec::new();
        self.skip_ws();
        if let Some('}') = self.chars.peek() {
            self.chars.next();
            return Ok(Value::Map(pairs));
        }
        loop {
            self.skip_ws();
            let key = self.parse_string()?;
            self.skip_ws();
            if self.chars.next() != Some(':') {
                return Err("expected ':'".into());
            }
            let val = self.parse_value()?;
            pairs.push((Value::Str(key), val));
            self.skip_ws();
            match self.chars.next() {
                Some(',') => {
                    self.skip_ws();
                }
                Some('}') => break,
                _ => return Err("expected ',' or '}'".into()),
            }
        }
        Ok(Value::Map(pairs))
    }
}
