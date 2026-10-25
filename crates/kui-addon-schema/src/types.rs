//! The schema's type language (ADR 0066, decision 2): a few primitives,
//! the schema's own enums and records, lists and optionals.

/// A prop's or a field's type.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Type {
    String,
    Number,
    Integer,
    Boolean,
    /// A colour (`0xRRGGBBAA`) or a `$token`.
    Color,
    /// Logical px or a `$token`.
    Length,
    /// A data channel's handle (ADR 0065).
    Channel,
    /// Any value: a message, a host's data.
    Any,
    /// An enum or a record the schema defines.
    Named(String),
    List(Box<Type>),
    Optional(Box<Type>),
}

impl Type {
    /// Parses `string`, `integer?`, `{Column}`, `{string}?`.
    pub fn parse(s: &str) -> Result<Type, String> {
        let s = s.trim();
        if let Some(inner) = s.strip_suffix('?') {
            let t = Type::parse(inner)?;
            if matches!(t, Type::Optional(_)) {
                return Err(format!("`{s}` is optional twice"));
            }
            return Ok(Type::Optional(Box::new(t)));
        }
        if let Some(inner) = s.strip_prefix('{').and_then(|r| r.strip_suffix('}')) {
            return Ok(Type::List(Box::new(Type::parse(inner)?)));
        }
        Ok(match s {
            "string" => Type::String,
            "number" => Type::Number,
            "integer" => Type::Integer,
            "boolean" => Type::Boolean,
            "color" => Type::Color,
            "length" => Type::Length,
            "channel" => Type::Channel,
            "any" => Type::Any,
            n if n.starts_with(|c: char| c.is_ascii_uppercase())
                && n.chars().all(|c| c.is_ascii_alphanumeric()) =>
            {
                Type::Named(n.to_string())
            }
            other => return Err(format!("`{other}` is no type (string, number, integer, boolean, color, length, channel, any, an enum or record, {{T}}, T?)")),
        })
    }

    /// The type without its `?`, and whether it had one.
    pub fn required(&self) -> (&Type, bool) {
        match self {
            Type::Optional(t) => (t, false),
            t => (t, true),
        }
    }

    /// The enums and records it names.
    pub fn names(&self) -> Vec<&str> {
        match self {
            Type::Named(n) => vec![n.as_str()],
            Type::List(t) | Type::Optional(t) => t.names(),
            _ => Vec::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn types_parse_and_refuse() {
        assert_eq!(Type::parse("integer").unwrap(), Type::Integer);
        assert_eq!(
            Type::parse("{Column}?").unwrap(),
            Type::Optional(Box::new(Type::List(Box::new(Type::Named("Column".into())))))
        );
        assert!(Type::parse("int").is_err());
        assert!(Type::parse("string??").is_err());
    }
}
