use crate::chunk::{Chunk, ExceptionEntry};
use crate::debug::LocalVarEntry;
use crate::gc::GcRef;
use crate::intern::{InternId, InternTable};
use crate::value::Value;
use std::collections::HashMap;
use std::fmt;
use std::sync::atomic::{AtomicU64, Ordering};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PendingToken(pub u64);

static PENDING_TOKEN_COUNTER: AtomicU64 = AtomicU64::new(1);

impl Default for PendingToken {
    fn default() -> Self {
        Self::new()
    }
}

impl PendingToken {
    pub fn new() -> Self {
        PendingToken(PENDING_TOKEN_COUNTER.fetch_add(1, Ordering::Relaxed))
    }
}

#[derive(Debug, Clone)]
pub enum NativeResult {
    Ready(Value),
    Pending(PendingToken),
    Error(String),
}

impl NativeResult {
    /// Convert to a Result for use with the ? operator in call sites.
    pub fn into_result(self) -> Result<Value, String> {
        match self {
            NativeResult::Ready(v) => Ok(v),
            NativeResult::Pending(_) => {
                Err("Pending result cannot be converted to value".to_string())
            }
            NativeResult::Error(e) => Err(e),
        }
    }
}

impl From<Result<Value, String>> for NativeResult {
    fn from(result: Result<Value, String>) -> Self {
        match result {
            Ok(v) => NativeResult::Ready(v),
            Err(e) => NativeResult::Error(e),
        }
    }
}

#[derive(Debug, Clone)]
pub enum NiObject {
    String(String),
    InternedString(InternId),
    List(Vec<Value>),
    Bytes(Vec<u8>),
    Map(Vec<(Value, Value)>), // ordered map
    Function(NiFunction),
    Closure(NiClosure),
    Upvalue(UpvalueObj),
    Class(NiClass),
    Instance(NiInstance),
    BoundMethod(BoundMethod),
    NativeFunction(NativeFn),
    NativeClass(NativeClass),
    Enum(NiEnum),
    Range(NiRange),
    Iterator(NiIterator),
    Fiber(GcRef), // placeholder reference to fiber
}

#[derive(Debug, Clone)]
pub struct NiFunction {
    pub name: String,
    pub arity: u8,
    pub default_count: u8,
    pub chunk: Chunk,
    pub upvalue_count: u8,
    pub exception_table: Vec<ExceptionEntry>,
    pub local_var_table: Vec<LocalVarEntry>,
    pub upvalue_names: Vec<String>,
    pub docstring: Option<String>,
}

#[derive(Debug, Clone)]
pub struct NiClosure {
    pub function: GcRef,
    pub upvalues: Vec<GcRef>,
}

#[derive(Debug, Clone)]
pub enum UpvalueObj {
    Open(usize), // stack slot
    Closed(Value),
}

#[derive(Debug, Clone)]
pub struct NiClass {
    pub name: String,
    pub methods: HashMap<InternId, GcRef>,
    pub superclass: Option<GcRef>,
    pub fields: HashMap<InternId, Value>, // default values
    pub docstring: Option<String>,
}

#[derive(Debug, Clone)]
pub struct NiInstance {
    pub class: GcRef,
    pub fields: HashMap<InternId, Value>,
}

#[derive(Debug, Clone)]
pub struct BoundMethod {
    pub receiver: Value,
    pub method: GcRef,
}

#[derive(Clone)]
pub struct NativeFn {
    pub name: String,
    pub arity: i8, // -1 = variadic
    pub function: fn(&[Value], &mut crate::gc::GcHeap, &InternTable) -> NativeResult,
}

impl fmt::Debug for NativeFn {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "<native fn {}>", self.name)
    }
}

#[derive(Clone)]
pub struct NativeClass {
    pub name: String,
    pub methods: HashMap<InternId, NativeFn>,
    pub static_methods: HashMap<InternId, NativeFn>,
}

impl fmt::Debug for NativeClass {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "<native class {}>", self.name)
    }
}

#[derive(Debug, Clone)]
pub struct NiEnum {
    pub name: String,
    pub variants: HashMap<InternId, Value>,
}

#[derive(Debug, Clone)]
pub struct NiRange {
    pub start: i64,
    pub end: i64,
    pub inclusive: bool,
    pub step: i64,
}

#[derive(Debug, Clone)]
pub enum NiIterator {
    Range {
        current: i64,
        end: i64,
        inclusive: bool,
        step: i64,
    },
    List {
        list: GcRef,
        index: usize,
    },
    Map {
        map: GcRef,
        index: usize,
    },
    String {
        string: GcRef,
        index: usize,
    },
    Bytes {
        bytes: GcRef,
        index: usize,
    },
}

impl NiObject {
    pub fn as_string(&self) -> Option<&str> {
        match self {
            NiObject::String(s) => Some(s),
            _ => None,
        }
    }
    pub fn as_string_with_intern<'a>(&'a self, interner: &'a InternTable) -> Option<&'a str> {
        match self {
            NiObject::String(s) => Some(s),
            NiObject::InternedString(id) => Some(interner.resolve(*id)),
            _ => None,
        }
    }
    pub fn as_intern_id(&self) -> Option<InternId> {
        match self {
            NiObject::InternedString(id) => Some(*id),
            _ => None,
        }
    }
    pub fn as_list(&self) -> Option<&Vec<Value>> {
        match self {
            NiObject::List(l) => Some(l),
            _ => None,
        }
    }
    pub fn as_list_mut(&mut self) -> Option<&mut Vec<Value>> {
        match self {
            NiObject::List(l) => Some(l),
            _ => None,
        }
    }
    pub fn as_map(&self) -> Option<&Vec<(Value, Value)>> {
        match self {
            NiObject::Map(m) => Some(m),
            _ => None,
        }
    }
    pub fn as_map_mut(&mut self) -> Option<&mut Vec<(Value, Value)>> {
        match self {
            NiObject::Map(m) => Some(m),
            _ => None,
        }
    }
    pub fn as_bytes(&self) -> Option<&Vec<u8>> {
        match self {
            NiObject::Bytes(b) => Some(b),
            _ => None,
        }
    }
    pub fn as_bytes_mut(&mut self) -> Option<&mut Vec<u8>> {
        match self {
            NiObject::Bytes(b) => Some(b),
            _ => None,
        }
    }
    pub fn as_function(&self) -> Option<&NiFunction> {
        match self {
            NiObject::Function(f) => Some(f),
            _ => None,
        }
    }
    pub fn as_closure(&self) -> Option<&NiClosure> {
        match self {
            NiObject::Closure(c) => Some(c),
            _ => None,
        }
    }
    pub fn as_class(&self) -> Option<&NiClass> {
        match self {
            NiObject::Class(c) => Some(c),
            _ => None,
        }
    }
    pub fn as_class_mut(&mut self) -> Option<&mut NiClass> {
        match self {
            NiObject::Class(c) => Some(c),
            _ => None,
        }
    }
    pub fn as_instance(&self) -> Option<&NiInstance> {
        match self {
            NiObject::Instance(i) => Some(i),
            _ => None,
        }
    }
    pub fn as_instance_mut(&mut self) -> Option<&mut NiInstance> {
        match self {
            NiObject::Instance(i) => Some(i),
            _ => None,
        }
    }
    pub fn as_enum(&self) -> Option<&NiEnum> {
        match self {
            NiObject::Enum(e) => Some(e),
            _ => None,
        }
    }
    pub fn as_upvalue(&self) -> Option<&UpvalueObj> {
        match self {
            NiObject::Upvalue(u) => Some(u),
            _ => None,
        }
    }
    pub fn as_upvalue_mut(&mut self) -> Option<&mut UpvalueObj> {
        match self {
            NiObject::Upvalue(u) => Some(u),
            _ => None,
        }
    }
    pub fn as_range(&self) -> Option<&NiRange> {
        match self {
            NiObject::Range(r) => Some(r),
            _ => None,
        }
    }
    pub fn as_iterator_mut(&mut self) -> Option<&mut NiIterator> {
        match self {
            NiObject::Iterator(i) => Some(i),
            _ => None,
        }
    }
    pub fn as_native(&self) -> Option<&NativeFn> {
        match self {
            NiObject::NativeFunction(n) => Some(n),
            _ => None,
        }
    }
    pub fn as_native_class(&self) -> Option<&NativeClass> {
        match self {
            NiObject::NativeClass(c) => Some(c),
            _ => None,
        }
    }
    pub fn as_bound_method(&self) -> Option<&BoundMethod> {
        match self {
            NiObject::BoundMethod(b) => Some(b),
            _ => None,
        }
    }

    pub fn is_falsy(&self) -> bool {
        match self {
            NiObject::String(s) => s.is_empty(),
            NiObject::InternedString(_) => false, // checked by VM with interner access
            NiObject::List(l) => l.is_empty(),
            NiObject::Bytes(b) => b.is_empty(),
            NiObject::Map(m) => m.is_empty(),
            _ => false,
        }
    }

    pub fn type_name(&self) -> &str {
        match self {
            NiObject::String(_) | NiObject::InternedString(_) => "string",
            NiObject::List(_) => "list",
            NiObject::Bytes(_) => "bytes",
            NiObject::Map(_) => "map",
            NiObject::Function(_) => "function",
            NiObject::Closure(_) => "closure",
            NiObject::Upvalue(_) => "upvalue",
            NiObject::Class(c) => &c.name,
            NiObject::Instance(_) => "instance",
            NiObject::BoundMethod(_) => "bound_method",
            NiObject::NativeFunction(n) => &n.name,
            NiObject::NativeClass(c) => &c.name,
            NiObject::Enum(e) => &e.name,
            NiObject::Range(_) => "range",
            NiObject::Iterator(_) => "iterator",
            NiObject::Fiber(_) => "fiber",
        }
    }

    /// Estimate the heap bytes owned by this object (not counting GcSlot overhead).
    pub fn size_bytes(&self) -> usize {
        match self {
            NiObject::String(s) => s.capacity(),
            NiObject::InternedString(_) => 0, // shared, not owned
            NiObject::List(v) => v.len() * std::mem::size_of::<Value>(),
            NiObject::Map(v) => v.len() * std::mem::size_of::<(Value, Value)>(),
            NiObject::Bytes(b) => b.capacity(),
            _ => std::mem::size_of::<NiObject>(),
        }
    }

    pub fn references(&self) -> Vec<GcRef> {
        match self {
            NiObject::List(items) => items.iter().filter_map(|v| v.as_object()).collect(),
            NiObject::Map(entries) => entries
                .iter()
                .flat_map(|(k, v)| [k.as_object(), v.as_object()])
                .flatten()
                .collect(),
            NiObject::Closure(c) => {
                let mut refs = vec![c.function];
                refs.extend(&c.upvalues);
                refs
            }
            NiObject::Instance(i) => {
                let mut refs = vec![i.class];
                refs.extend(i.fields.values().filter_map(|v| v.as_object()));
                refs
            }
            NiObject::BoundMethod(b) => {
                let mut refs = vec![b.method];
                if let Some(r) = b.receiver.as_object() {
                    refs.push(r);
                }
                refs
            }
            NiObject::Class(c) => {
                let mut refs: Vec<GcRef> = c.methods.values().cloned().collect();
                if let Some(s) = c.superclass {
                    refs.push(s);
                }
                refs.extend(c.fields.values().filter_map(|v| v.as_object()));
                refs
            }
            NiObject::Upvalue(UpvalueObj::Closed(v)) => v.as_object().into_iter().collect(),
            NiObject::Iterator(NiIterator::List { list, .. }) => vec![*list],
            NiObject::Iterator(NiIterator::Map { map, .. }) => vec![*map],
            NiObject::Iterator(NiIterator::String { string, .. }) => vec![*string],
            NiObject::Iterator(NiIterator::Bytes { bytes, .. }) => vec![*bytes],
            NiObject::Fiber(r) => vec![*r],
            NiObject::Function(f) => f
                .chunk
                .constants
                .iter()
                .filter_map(|v| v.as_object())
                .collect(),
            NiObject::NativeClass(_) => Vec::new(),
            NiObject::InternedString(_) => Vec::new(),
            _ => Vec::new(),
        }
    }

    pub fn display(&self, heap: &crate::gc::GcHeap, interner: &InternTable) -> String {
        display_inner(self, heap, interner, 0)
    }
}

const DISPLAY_MAX_DEPTH: usize = 32;

fn display_inner(
    obj: &NiObject,
    heap: &crate::gc::GcHeap,
    interner: &InternTable,
    depth: usize,
) -> String {
    if depth > DISPLAY_MAX_DEPTH {
        return "...".to_string();
    }
    match obj {
        NiObject::String(s) => s.clone(),
        NiObject::InternedString(id) => interner.resolve(*id).to_string(),
        NiObject::Bytes(b) => format!("<bytes len={}>", b.len()),
        NiObject::List(items) => {
            let parts: Vec<String> = items
                .iter()
                .map(|v| value_display_inner(v, heap, interner, depth + 1))
                .collect();
            format!("[{}]", parts.join(", "))
        }
        NiObject::Map(entries) => {
            let parts: Vec<String> = entries
                .iter()
                .map(|(k, v)| {
                    format!(
                        "{}: {}",
                        value_display_inner(k, heap, interner, depth + 1),
                        value_display_inner(v, heap, interner, depth + 1)
                    )
                })
                .collect();
            format!("[{}]", parts.join(", "))
        }
        NiObject::Function(f) => format!("<fn {}>", f.name),
        NiObject::Closure(c) => {
            if let Some(obj) = heap.get(c.function) {
                if let Some(f) = obj.as_function() {
                    return format!("<fn {}>", f.name);
                }
            }
            "<closure>".to_string()
        }
        NiObject::Class(c) => format!("<class {}>", c.name),
        NiObject::Instance(i) => {
            if let Some(obj) = heap.get(i.class) {
                if let Some(c) = obj.as_class() {
                    return format!("<{} instance>", c.name);
                }
            }
            "<instance>".to_string()
        }
        NiObject::NativeFunction(n) => format!("<native fn {}>", n.name),
        NiObject::NativeClass(c) => format!("<class {}>", c.name),
        NiObject::Enum(e) => format!("<enum {}>", e.name),
        NiObject::Range(r) => {
            if r.inclusive {
                format!("{}..={}", r.start, r.end)
            } else {
                format!("{}..{}", r.start, r.end)
            }
        }
        NiObject::BoundMethod(_) => "<bound method>".to_string(),
        NiObject::Upvalue(_) => "<upvalue>".to_string(),
        NiObject::Iterator(_) => "<iterator>".to_string(),
        NiObject::Fiber(_) => "<fiber>".to_string(),
    }
}

pub fn value_display(val: &Value, heap: &crate::gc::GcHeap, interner: &InternTable) -> String {
    value_display_inner(val, heap, interner, 0)
}

fn value_display_inner(
    val: &Value,
    heap: &crate::gc::GcHeap,
    interner: &InternTable,
    depth: usize,
) -> String {
    match val {
        Value::Object(r) => {
            if let Some(obj) = heap.get(*r) {
                // Strings should be quoted in collection display
                match obj {
                    NiObject::String(s) => format!("\"{}\"", s),
                    NiObject::InternedString(id) => format!("\"{}\"", interner.resolve(*id)),
                    _ => display_inner(obj, heap, interner, depth),
                }
            } else {
                "<freed>".to_string()
            }
        }
        _ => format!("{}", val),
    }
}
