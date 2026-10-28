//! A strict JSON reader into [`Value`], for the files the core reads
//! itself — a DTCG token file and its resolver (ADR 0062). RFC 8259 and
//! nothing more: no comments, no trailing commas. An object keeps its keys
//! in file order (a [`Value::Map`]), and a key given twice keeps the last
//! value, as the DTCG resolver's "last declaration wins" reads it. A
//! number with no fraction or exponent that fits an `i64` is an
//! [`Value::Int`]; anything else is a [`Value::Float`].

use crate::Value;

/// Where a document stopped being JSON: 1-based line and column, and what
/// was expected there.
#[derive(Clone, Debug, PartialEq)]
pub struct JsonError {
    pub line: usize,
    pub col: usize,
    pub message: String,
}

impl std::fmt::Display for JsonError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}:{}: {}", self.line, self.col, self.message)
    }
}

impl std::error::Error for JsonError {}

/// Nesting deeper than this is refused rather than recursed into: a token
/// file is a few levels of groups, and a hostile one should not take the
/// stack with it.
const MAX_DEPTH: usize = 128;

/// Reads one JSON document. Whitespace around it is fine; anything else
/// after it is an error.
pub fn parse(src: &str) -> Result<Value, JsonError> {
    let mut p = Parser {
        src: src.as_bytes(),
        at: 0,
    };
    p.ws();
    let v = p.value(0)?;
    p.ws();
    if p.at < p.src.len() {
        return Err(p.err("expected the end of the document"));
    }
    Ok(v)
}

struct Parser<'a> {
    src: &'a [u8],
    at: usize,
}

impl Parser<'_> {
    fn err(&self, message: impl Into<String>) -> JsonError {
        let before = &self.src[..self.at.min(self.src.len())];
        let line = before.iter().filter(|b| **b == b'\n').count() + 1;
        let col = before.iter().rev().take_while(|b| **b != b'\n').count() + 1;
        JsonError {
            line,
            col,
            message: message.into(),
        }
    }

    fn peek(&self) -> Option<u8> {
        self.src.get(self.at).copied()
    }

    fn ws(&mut self) {
        while matches!(self.peek(), Some(b' ' | b'\t' | b'\n' | b'\r')) {
            self.at += 1;
        }
    }

    fn eat(&mut self, lit: &str) -> Result<(), JsonError> {
        if self.src[self.at..].starts_with(lit.as_bytes()) {
            self.at += lit.len();
            Ok(())
        } else {
            Err(self.err(format!("expected `{lit}`")))
        }
    }

    fn value(&mut self, depth: usize) -> Result<Value, JsonError> {
        if depth > MAX_DEPTH {
            return Err(self.err("nested too deeply"));
        }
        match self.peek() {
            Some(b'{') => self.object(depth),
            Some(b'[') => self.array(depth),
            Some(b'"') => Ok(Value::Str(self.string()?)),
            Some(b't') => self.eat("true").map(|_| Value::Bool(true)),
            Some(b'f') => self.eat("false").map(|_| Value::Bool(false)),
            Some(b'n') => self.eat("null").map(|_| Value::Null),
            Some(b'-' | b'0'..=b'9') => self.number(),
            Some(_) => Err(self.err("expected a value")),
            None => Err(self.err("expected a value, found the end")),
        }
    }

    fn object(&mut self, depth: usize) -> Result<Value, JsonError> {
        self.at += 1;
        let mut out: Vec<(String, Value)> = Vec::new();
        self.ws();
        if self.peek() == Some(b'}') {
            self.at += 1;
            return Ok(Value::Map(out));
        }
        loop {
            self.ws();
            if self.peek() != Some(b'"') {
                return Err(self.err("expected a key in double quotes"));
            }
            let key = self.string()?;
            self.ws();
            self.eat(":")?;
            self.ws();
            let v = self.value(depth + 1)?;
            match out.iter_mut().find(|(k, _)| *k == key) {
                Some(slot) => slot.1 = v,
                None => out.push((key, v)),
            }
            self.ws();
            match self.peek() {
                Some(b',') => self.at += 1,
                Some(b'}') => {
                    self.at += 1;
                    return Ok(Value::Map(out));
                }
                _ => return Err(self.err("expected `,` or `}`")),
            }
        }
    }

    fn array(&mut self, depth: usize) -> Result<Value, JsonError> {
        self.at += 1;
        let mut out = Vec::new();
        self.ws();
        if self.peek() == Some(b']') {
            self.at += 1;
            return Ok(Value::List(out));
        }
        loop {
            self.ws();
            out.push(self.value(depth + 1)?);
            self.ws();
            match self.peek() {
                Some(b',') => self.at += 1,
                Some(b']') => {
                    self.at += 1;
                    return Ok(Value::List(out));
                }
                _ => return Err(self.err("expected `,` or `]`")),
            }
        }
    }

    fn string(&mut self) -> Result<String, JsonError> {
        self.at += 1;
        let mut out = String::new();
        loop {
            let start = self.at;
            while let Some(b) = self.peek() {
                if b == b'"' || b == b'\\' || b < 0x20 {
                    break;
                }
                self.at += 1;
            }
            // The run is valid UTF-8: it came from a `&str` and stops only
            // at ASCII bytes.
            out.push_str(std::str::from_utf8(&self.src[start..self.at]).unwrap_or(""));
            match self.peek() {
                Some(b'"') => {
                    self.at += 1;
                    return Ok(out);
                }
                Some(b'\\') => {
                    self.at += 1;
                    let c = match self.peek() {
                        Some(b'"') => '"',
                        Some(b'\\') => '\\',
                        Some(b'/') => '/',
                        Some(b'b') => '\u{8}',
                        Some(b'f') => '\u{c}',
                        Some(b'n') => '\n',
                        Some(b'r') => '\r',
                        Some(b't') => '\t',
                        Some(b'u') => {
                            self.at += 1;
                            let hi = self.hex4()?;
                            let code = if (0xD800..0xDC00).contains(&hi) {
                                self.eat("\\u")?;
                                let lo = self.hex4()?;
                                if !(0xDC00..0xE000).contains(&lo) {
                                    return Err(self.err("expected a low surrogate"));
                                }
                                0x10000 + ((hi - 0xD800) << 10) + (lo - 0xDC00)
                            } else {
                                hi
                            };
                            out.push(
                                char::from_u32(code).ok_or_else(|| self.err("not a character"))?,
                            );
                            continue;
                        }
                        _ => return Err(self.err("expected an escape")),
                    };
                    out.push(c);
                    self.at += 1;
                }
                Some(_) => return Err(self.err("a control character in a string")),
                None => return Err(self.err("an unterminated string")),
            }
        }
    }

    fn hex4(&mut self) -> Result<u32, JsonError> {
        let digits = self
            .src
            .get(self.at..self.at + 4)
            .ok_or_else(|| self.err("expected four hex digits"))?;
        let s = std::str::from_utf8(digits).map_err(|_| self.err("expected four hex digits"))?;
        let v = u32::from_str_radix(s, 16).map_err(|_| self.err("expected four hex digits"))?;
        self.at += 4;
        Ok(v)
    }

    fn number(&mut self) -> Result<Value, JsonError> {
        let start = self.at;
        if self.peek() == Some(b'-') {
            self.at += 1;
        }
        match self.peek() {
            Some(b'0') => self.at += 1,
            Some(b'1'..=b'9') => {
                while matches!(self.peek(), Some(b'0'..=b'9')) {
                    self.at += 1;
                }
            }
            _ => return Err(self.err("expected a digit")),
        }
        let mut integral = true;
        if self.peek() == Some(b'.') {
            integral = false;
            self.at += 1;
            if !matches!(self.peek(), Some(b'0'..=b'9')) {
                return Err(self.err("expected a digit after `.`"));
            }
            while matches!(self.peek(), Some(b'0'..=b'9')) {
                self.at += 1;
            }
        }
        if matches!(self.peek(), Some(b'e' | b'E')) {
            integral = false;
            self.at += 1;
            if matches!(self.peek(), Some(b'+' | b'-')) {
                self.at += 1;
            }
            if !matches!(self.peek(), Some(b'0'..=b'9')) {
                return Err(self.err("expected a digit in the exponent"));
            }
            while matches!(self.peek(), Some(b'0'..=b'9')) {
                self.at += 1;
            }
        }
        let text = std::str::from_utf8(&self.src[start..self.at]).unwrap_or("0");
        if integral && let Ok(i) = text.parse::<i64>() {
            return Ok(Value::Int(i));
        }
        text.parse::<f64>()
            .map(Value::Float)
            .map_err(|_| self.err("not a number"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_every_kind_and_keeps_key_order() {
        let v = parse(r#" { "b": [1, -2.5, 3e2, true, false, null], "a": "x\né😀", "b2": {} } "#)
            .unwrap();
        let Value::Map(m) = &v else { panic!() };
        assert_eq!(m[0].0, "b");
        assert_eq!(m[1].0, "a");
        assert_eq!(
            m[0].1,
            Value::List(vec![
                Value::Int(1),
                Value::Float(-2.5),
                Value::Float(300.0),
                Value::Bool(true),
                Value::Bool(false),
                Value::Null
            ])
        );
        assert_eq!(m[1].1, Value::Str("x\né😀".into()));
    }

    #[test]
    fn a_repeated_key_keeps_the_last_value() {
        let v = parse(r#"{"a": 1, "a": 2}"#).unwrap();
        assert_eq!(v, Value::Map(vec![("a".into(), Value::Int(2))]));
    }

    #[test]
    fn errors_say_where() {
        let e = parse("{\n  \"a\": 1,\n}").unwrap_err();
        assert_eq!((e.line, e.col), (3, 1), "{e}");
        assert!(parse("[1,]").is_err());
        assert!(parse("01").is_err());
        assert!(parse("\"a\" b").is_err());
        assert!(
            parse(&"[".repeat(200))
                .unwrap_err()
                .message
                .contains("deeply")
        );
    }
}
