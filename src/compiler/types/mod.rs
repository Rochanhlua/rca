use std::fmt;

#[derive(Debug, Clone, PartialEq)]
pub enum Type {
    Int,
    Float,
    Bool,
    String,
    Char,
    None,
    List(Box<Type>),
    Range,
    Tensor,
    /// A user-defined struct, identified by name.
    Struct(String),
    /// A user-defined enum, identified by name.
    Enum(String),
    /// Used after an error, or when a type is not yet known.
    /// It never causes further errors (prevents cascades).
    Unknown,
}

impl Type {
    pub fn from_name(name: &str) -> Option<Type> {
        match name {
            "Int" => Some(Type::Int),
            "Float" => Some(Type::Float),
            "Bool" => Some(Type::Bool),
            "String" => Some(Type::String),
            "Char" => Some(Type::Char),
            "None" => Some(Type::None),
            "Tensor" => Some(Type::Tensor),
            _ => None,
        }
    }

    pub fn compatible(expected: &Type, actual: &Type) -> bool {
        expected == actual || *expected == Type::Unknown || *actual == Type::Unknown
    }
}

impl fmt::Display for Type {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Type::Int => write!(f, "Int"),
            Type::Float => write!(f, "Float"),
            Type::Bool => write!(f, "Bool"),
            Type::String => write!(f, "String"),
            Type::Char => write!(f, "Char"),
            Type::None => write!(f, "None"),
            Type::List(inner) => write!(f, "List[{}]", inner),
            Type::Range => write!(f, "Range"),
            Type::Tensor => write!(f, "Tensor"),
            Type::Struct(name) => write!(f, "{}", name),
            Type::Enum(name) => write!(f, "{}", name),
            Type::Unknown => write!(f, "<unknown>"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn from_name_works() {
        assert_eq!(Type::from_name("Int"), Some(Type::Int));
        assert_eq!(Type::from_name("Foo"), None);
    }

    #[test]
    fn display_and_compatibility() {
        assert_eq!(Type::List(Box::new(Type::Int)).to_string(), "List[Int]");
        assert!(Type::compatible(&Type::Int, &Type::Int));
        assert!(!Type::compatible(&Type::Int, &Type::String));
        assert!(Type::compatible(&Type::Int, &Type::Unknown));
    }
}
