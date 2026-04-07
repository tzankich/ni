use crate::gc::GcRef;
use std::fmt;

#[derive(Debug, Clone)]
pub enum Value {
    Int(i64),
    Float(f64),
    Bool(bool),
    None,
    Object(GcRef),
}

impl Value {
    pub fn is_falsy(&self) -> bool {
        match self {
            Value::Bool(false) | Value::None => true,
            Value::Int(0) => true,
            Value::Float(f) => *f == 0.0,
            _ => false, // Object truthiness checked via object kind
        }
    }

    pub fn type_name(&self) -> &'static str {
        match self {
            Value::Int(_) => "int",
            Value::Float(_) => "float",
            Value::Bool(_) => "bool",
            Value::None => "none",
            Value::Object(_) => "object",
        }
    }

    pub fn as_int(&self) -> Option<i64> {
        match self {
            Value::Int(n) => Some(*n),
            _ => None,
        }
    }
    pub fn as_float(&self) -> Option<f64> {
        match self {
            Value::Float(f) => Some(*f),
            _ => None,
        }
    }
    pub fn as_bool(&self) -> Option<bool> {
        match self {
            Value::Bool(b) => Some(*b),
            _ => None,
        }
    }
    pub fn as_object(&self) -> Option<GcRef> {
        match self {
            Value::Object(r) => Some(*r),
            _ => None,
        }
    }
    pub fn is_none(&self) -> bool {
        matches!(self, Value::None)
    }

    pub fn to_number(&self) -> Option<f64> {
        match self {
            Value::Int(n) => Some(*n as f64),
            Value::Float(f) => Some(*f),
            _ => None,
        }
    }
}

impl fmt::Display for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Value::Int(n) => write!(f, "{}", n),
            Value::Float(n) => {
                if *n == n.floor() && n.is_finite() {
                    write!(f, "{:.1}", n)
                } else {
                    write!(f, "{}", n)
                }
            }
            Value::Bool(b) => write!(f, "{}", b),
            Value::None => write!(f, "none"),
            Value::Object(_) => write!(f, "<object>"),
        }
    }
}

impl PartialEq for Value {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Value::Int(a), Value::Int(b)) => a == b,
            (Value::Float(a), Value::Float(b)) => a == b,
            (Value::Int(a), Value::Float(b)) => *a as f64 == *b,
            (Value::Float(a), Value::Int(b)) => *a == *b as f64,
            (Value::Bool(a), Value::Bool(b)) => a == b,
            (Value::None, Value::None) => true,
            (Value::Object(a), Value::Object(b)) => a == b,
            _ => false,
        }
    }
}

/// Compare two values with heap-aware string content comparison.
/// Unlike PartialEq, this compares string objects by content, not by GcRef identity.
pub fn values_equal(
    a: &Value,
    b: &Value,
    heap: &crate::gc::GcHeap,
    interner: &crate::intern::InternTable,
) -> bool {
    match (a, b) {
        (Value::Object(a_ref), Value::Object(b_ref)) => {
            if a_ref == b_ref {
                return true;
            }
            let a_obj = heap.get(*a_ref);
            let b_obj = heap.get(*b_ref);
            match (a_obj, b_obj) {
                (Some(a_o), Some(b_o)) => {
                    let a_str = a_o.as_string_with_intern(interner);
                    let b_str = b_o.as_string_with_intern(interner);
                    match (a_str, b_str) {
                        (Some(a), Some(b)) => a == b,
                        _ => false,
                    }
                }
                _ => false,
            }
        }
        _ => a == b,
    }
}
