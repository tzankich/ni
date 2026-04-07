use std::cell::RefCell;
use std::collections::HashMap;
use std::fmt;
use std::rc::Rc;

use crate::class::{NiClassDef, NiInstance};

#[derive(Debug, Clone)]
pub enum NiValue {
    Int(i64),
    Float(f64),
    Bool(bool),
    None,
    String(Rc<String>),
    List(Rc<RefCell<Vec<NiValue>>>),
    Map(Rc<RefCell<Vec<(NiValue, NiValue)>>>),
    Instance(Rc<RefCell<NiInstance>>),
    Function(NiFunctionRef),
    Class(Rc<NiClassDef>),
    Enum(Rc<NiEnumDef>),
    Range(NiRange),
}

#[derive(Clone)]
pub struct NiFunctionRef {
    pub name: String,
    pub func:
        Rc<dyn Fn(&mut dyn crate::vm_context::NiVm, &[NiValue]) -> crate::error::NiResult<NiValue>>,
}

impl fmt::Debug for NiFunctionRef {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("NiFunctionRef")
            .field("name", &self.name)
            .finish()
    }
}

#[derive(Debug, Clone)]
pub struct NiEnumDef {
    pub name: String,
    pub variants: HashMap<String, NiValue>,
}

#[derive(Debug, Clone)]
pub struct NiRange {
    pub start: i64,
    pub end: i64,
    pub inclusive: bool,
    pub step: i64,
}

impl PartialEq for NiValue {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (NiValue::Int(a), NiValue::Int(b)) => a == b,
            (NiValue::Float(a), NiValue::Float(b)) => a == b,
            (NiValue::Int(a), NiValue::Float(b)) => (*a as f64) == *b,
            (NiValue::Float(a), NiValue::Int(b)) => *a == (*b as f64),
            (NiValue::Bool(a), NiValue::Bool(b)) => a == b,
            (NiValue::None, NiValue::None) => true,
            (NiValue::String(a), NiValue::String(b)) => a == b,
            _ => false,
        }
    }
}

impl NiValue {
    pub fn is_truthy(&self) -> bool {
        match self {
            NiValue::Bool(b) => *b,
            NiValue::None => false,
            NiValue::Int(n) => *n != 0,
            NiValue::Float(f) => *f != 0.0,
            NiValue::String(s) => !s.is_empty(),
            NiValue::List(l) => !l.borrow().is_empty(),
            _ => true,
        }
    }

    pub fn is_none(&self) -> bool {
        matches!(self, NiValue::None)
    }

    pub fn type_name(&self) -> &'static str {
        match self {
            NiValue::Int(_) => "Int",
            NiValue::Float(_) => "Float",
            NiValue::Bool(_) => "Bool",
            NiValue::None => "None",
            NiValue::String(_) => "String",
            NiValue::List(_) => "List",
            NiValue::Map(_) => "Map",
            NiValue::Instance(_) => "Instance",
            NiValue::Function(_) => "Function",
            NiValue::Class(_) => "Class",
            NiValue::Enum(_) => "Enum",
            NiValue::Range(_) => "Range",
        }
    }

    pub fn to_display_string(&self) -> String {
        match self {
            NiValue::Int(n) => n.to_string(),
            NiValue::Float(f) => {
                if f.fract() == 0.0 {
                    format!("{:.1}", f)
                } else {
                    f.to_string()
                }
            }
            NiValue::Bool(b) => {
                if *b {
                    "true".into()
                } else {
                    "false".into()
                }
            }
            NiValue::None => "none".into(),
            NiValue::String(s) => s.as_ref().clone(),
            NiValue::List(l) => {
                let items: Vec<String> = l.borrow().iter().map(|v| v.to_display_string()).collect();
                format!("[{}]", items.join(", "))
            }
            NiValue::Map(m) => {
                let pairs: Vec<String> = m
                    .borrow()
                    .iter()
                    .map(|(k, v)| format!("{}: {}", k.to_display_string(), v.to_display_string()))
                    .collect();
                format!("{{{}}}", pairs.join(", "))
            }
            NiValue::Instance(inst) => {
                format!("<instance {}>", inst.borrow().class_name)
            }
            NiValue::Function(f) => format!("<fn {}>", f.name),
            NiValue::Class(c) => format!("<class {}>", c.name),
            NiValue::Enum(e) => format!("<enum {}>", e.name),
            NiValue::Range(r) => {
                if r.inclusive {
                    format!("{}..={}", r.start, r.end)
                } else {
                    format!("{}..{}", r.start, r.end)
                }
            }
        }
    }
}

impl fmt::Display for NiValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.to_display_string())
    }
}
