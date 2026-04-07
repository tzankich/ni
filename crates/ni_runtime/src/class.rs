use std::collections::HashMap;
use std::rc::Rc;

use crate::error::{NiResult, NiRuntimeError};
use crate::value::NiValue;
use crate::vm_context::NiVm;

pub type NiMethodFn = Rc<dyn Fn(&mut dyn NiVm, &NiValue, &[NiValue]) -> NiResult<NiValue>>;

#[derive(Clone)]
pub struct NiClassDef {
    pub name: String,
    pub superclass: Option<Rc<NiClassDef>>,
    pub methods: HashMap<String, NiMethodFn>,
    pub static_methods: HashMap<String, NiMethodFn>,
    pub default_fields: HashMap<String, NiValue>,
}

impl std::fmt::Debug for NiClassDef {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("NiClassDef")
            .field("name", &self.name)
            .field("methods", &self.methods.keys().collect::<Vec<_>>())
            .finish()
    }
}

impl NiClassDef {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            superclass: None,
            methods: HashMap::new(),
            static_methods: HashMap::new(),
            default_fields: HashMap::new(),
        }
    }

    pub fn find_method(&self, name: &str) -> Option<&NiMethodFn> {
        self.methods
            .get(name)
            .or_else(|| self.superclass.as_ref().and_then(|sc| sc.find_method(name)))
    }
}

#[derive(Debug, Clone)]
pub struct NiInstance {
    pub class_name: String,
    pub class: Rc<NiClassDef>,
    pub fields: HashMap<String, NiValue>,
}

impl NiInstance {
    pub fn new(class: Rc<NiClassDef>) -> Self {
        let mut fields = HashMap::new();
        // Copy default field values
        for (name, value) in &class.default_fields {
            fields.insert(name.clone(), value.clone());
        }
        // Also copy from superclass chain
        let mut sc = class.superclass.as_ref();
        while let Some(super_class) = sc {
            for (name, value) in &super_class.default_fields {
                fields.entry(name.clone()).or_insert_with(|| value.clone());
            }
            sc = super_class.superclass.as_ref();
        }
        Self {
            class_name: class.name.clone(),
            class,
            fields,
        }
    }
}

pub fn ni_get_prop(value: &NiValue, name: &str) -> NiResult<NiValue> {
    match value {
        NiValue::Instance(inst) => {
            let inst = inst.borrow();
            // Check fields first
            if let Some(val) = inst.fields.get(name) {
                return Ok(val.clone());
            }
            // Check methods (return as bound method placeholder)
            if inst.class.find_method(name).is_some() {
                return Ok(NiValue::String(std::rc::Rc::new(format!(
                    "<bound method {}>",
                    name
                ))));
            }
            Err(NiRuntimeError::new(format!(
                "Property '{}' not found on instance of '{}'",
                name, inst.class_name
            )))
        }
        NiValue::Map(m) => {
            let key = NiValue::String(std::rc::Rc::new(name.to_string()));
            let map = m.borrow();
            for (k, v) in map.iter() {
                if k == &key {
                    return Ok(v.clone());
                }
            }
            Err(NiRuntimeError::new(format!(
                "Key '{}' not found in map",
                name
            )))
        }
        NiValue::Enum(e) => {
            if let Some(val) = e.variants.get(name) {
                return Ok(val.clone());
            }
            Err(NiRuntimeError::new(format!(
                "Variant '{}' not found on enum '{}'",
                name, e.name
            )))
        }
        _ => Err(NiRuntimeError::new(format!(
            "Cannot access property '{}' on {}",
            name,
            value.type_name()
        ))),
    }
}

pub fn ni_set_prop(value: &NiValue, name: &str, new_val: NiValue) -> NiResult<()> {
    match value {
        NiValue::Instance(inst) => {
            inst.borrow_mut().fields.insert(name.to_string(), new_val);
            Ok(())
        }
        _ => Err(NiRuntimeError::new(format!(
            "Cannot set property '{}' on {}",
            name,
            value.type_name()
        ))),
    }
}

pub fn ni_method_call(
    vm: &mut dyn NiVm,
    receiver: &NiValue,
    method_name: &str,
    args: &[NiValue],
) -> NiResult<NiValue> {
    match receiver {
        NiValue::Instance(inst) => {
            let method = {
                let inst = inst.borrow();
                inst.class.find_method(method_name).cloned()
            };
            if let Some(method) = method {
                method(vm, receiver, args)
            } else {
                Err(NiRuntimeError::new(format!(
                    "Method '{}' not found on '{}'",
                    method_name,
                    inst.borrow().class_name
                )))
            }
        }
        NiValue::List(list) => ni_list_method(list, method_name, args),
        NiValue::String(s) => ni_string_method(s, method_name, args),
        NiValue::Map(map) => ni_map_method(map, method_name, args),
        _ => Err(NiRuntimeError::new(format!(
            "Cannot call method '{}' on {}",
            method_name,
            receiver.type_name()
        ))),
    }
}

fn ni_list_method(
    list: &std::rc::Rc<std::cell::RefCell<Vec<NiValue>>>,
    method: &str,
    args: &[NiValue],
) -> NiResult<NiValue> {
    match method {
        "append" | "push" => {
            if let Some(val) = args.first() {
                list.borrow_mut().push(val.clone());
                Ok(NiValue::None)
            } else {
                Err(NiRuntimeError::new("append() requires 1 argument"))
            }
        }
        "pop" => {
            let val = list.borrow_mut().pop().unwrap_or(NiValue::None);
            Ok(val)
        }
        "len" | "length" => Ok(NiValue::Int(list.borrow().len() as i64)),
        "contains" => {
            if let Some(val) = args.first() {
                let found = list.borrow().iter().any(|v| v == val);
                Ok(NiValue::Bool(found))
            } else {
                Err(NiRuntimeError::new("contains() requires 1 argument"))
            }
        }
        "remove" => {
            if let Some(NiValue::Int(idx)) = args.first() {
                let mut l = list.borrow_mut();
                let idx = *idx as usize;
                if idx < l.len() {
                    Ok(l.remove(idx))
                } else {
                    Err(NiRuntimeError::new("Index out of bounds"))
                }
            } else {
                Err(NiRuntimeError::new("remove() requires an Int argument"))
            }
        }
        _ => Err(NiRuntimeError::new(format!(
            "Unknown list method '{}'",
            method
        ))),
    }
}

fn ni_string_method(s: &std::rc::Rc<String>, method: &str, args: &[NiValue]) -> NiResult<NiValue> {
    match method {
        "len" | "length" => Ok(NiValue::Int(s.len() as i64)),
        "upper" | "to_upper" => Ok(NiValue::String(std::rc::Rc::new(s.to_uppercase()))),
        "lower" | "to_lower" => Ok(NiValue::String(std::rc::Rc::new(s.to_lowercase()))),
        "contains" => {
            if let Some(NiValue::String(substr)) = args.first() {
                Ok(NiValue::Bool(s.contains(substr.as_str())))
            } else {
                Err(NiRuntimeError::new("contains() requires a String argument"))
            }
        }
        "split" => {
            if let Some(NiValue::String(sep)) = args.first() {
                let parts: Vec<NiValue> = s
                    .split(sep.as_str())
                    .map(|p| NiValue::String(std::rc::Rc::new(p.to_string())))
                    .collect();
                Ok(NiValue::List(std::rc::Rc::new(std::cell::RefCell::new(
                    parts,
                ))))
            } else {
                Err(NiRuntimeError::new("split() requires a String argument"))
            }
        }
        "trim" => Ok(NiValue::String(std::rc::Rc::new(s.trim().to_string()))),
        "starts_with" => {
            if let Some(NiValue::String(prefix)) = args.first() {
                Ok(NiValue::Bool(s.starts_with(prefix.as_str())))
            } else {
                Err(NiRuntimeError::new(
                    "starts_with() requires a String argument",
                ))
            }
        }
        "ends_with" => {
            if let Some(NiValue::String(suffix)) = args.first() {
                Ok(NiValue::Bool(s.ends_with(suffix.as_str())))
            } else {
                Err(NiRuntimeError::new(
                    "ends_with() requires a String argument",
                ))
            }
        }
        _ => Err(NiRuntimeError::new(format!(
            "Unknown string method '{}'",
            method
        ))),
    }
}

fn ni_map_method(
    map: &std::rc::Rc<std::cell::RefCell<Vec<(NiValue, NiValue)>>>,
    method: &str,
    args: &[NiValue],
) -> NiResult<NiValue> {
    match method {
        "len" | "length" => Ok(NiValue::Int(map.borrow().len() as i64)),
        "keys" => {
            let keys: Vec<NiValue> = map.borrow().iter().map(|(k, _)| k.clone()).collect();
            Ok(NiValue::List(std::rc::Rc::new(std::cell::RefCell::new(
                keys,
            ))))
        }
        "values" => {
            let values: Vec<NiValue> = map.borrow().iter().map(|(_, v)| v.clone()).collect();
            Ok(NiValue::List(std::rc::Rc::new(std::cell::RefCell::new(
                values,
            ))))
        }
        "contains_key" | "has_key" => {
            if let Some(key) = args.first() {
                let found = map.borrow().iter().any(|(k, _)| k == key);
                Ok(NiValue::Bool(found))
            } else {
                Err(NiRuntimeError::new("contains_key() requires 1 argument"))
            }
        }
        "remove" => {
            if let Some(key) = args.first() {
                let mut m = map.borrow_mut();
                if let Some(pos) = m.iter().position(|(k, _)| k == key) {
                    let (_, val) = m.remove(pos);
                    Ok(val)
                } else {
                    Ok(NiValue::None)
                }
            } else {
                Err(NiRuntimeError::new("remove() requires 1 argument"))
            }
        }
        _ => Err(NiRuntimeError::new(format!(
            "Unknown map method '{}'",
            method
        ))),
    }
}
