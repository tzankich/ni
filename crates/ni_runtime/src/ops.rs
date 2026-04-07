use std::cell::RefCell;
use std::rc::Rc;

use crate::error::{NiResult, NiRuntimeError};
use crate::value::NiValue;

// Arithmetic operations

pub fn ni_add(left: &NiValue, right: &NiValue) -> NiResult<NiValue> {
    match (left, right) {
        (NiValue::Int(a), NiValue::Int(b)) => Ok(NiValue::Int(a.checked_add(*b).ok_or_else(|| NiRuntimeError::new("Integer overflow"))?)),
        (NiValue::Float(a), NiValue::Float(b)) => Ok(NiValue::Float(a + b)),
        (NiValue::Int(a), NiValue::Float(b)) => Ok(NiValue::Float(*a as f64 + b)),
        (NiValue::Float(a), NiValue::Int(b)) => Ok(NiValue::Float(a + *b as f64)),
        (NiValue::String(a), NiValue::String(b)) => {
            Ok(NiValue::String(Rc::new(format!("{}{}", a, b))))
        }
        (NiValue::String(a), other) => Ok(NiValue::String(Rc::new(format!(
            "{}{}",
            a,
            other.to_display_string()
        )))),
        (NiValue::List(a), NiValue::List(b)) => {
            let mut new_list = a.borrow().clone();
            new_list.extend(b.borrow().iter().cloned());
            Ok(NiValue::List(Rc::new(RefCell::new(new_list))))
        }
        _ => Err(NiRuntimeError::new(format!(
            "Cannot add {} and {}",
            left.type_name(),
            right.type_name()
        ))),
    }
}

pub fn ni_sub(left: &NiValue, right: &NiValue) -> NiResult<NiValue> {
    match (left, right) {
        (NiValue::Int(a), NiValue::Int(b)) => Ok(NiValue::Int(a.checked_sub(*b).ok_or_else(|| NiRuntimeError::new("Integer overflow"))?)),
        (NiValue::Float(a), NiValue::Float(b)) => Ok(NiValue::Float(a - b)),
        (NiValue::Int(a), NiValue::Float(b)) => Ok(NiValue::Float(*a as f64 - b)),
        (NiValue::Float(a), NiValue::Int(b)) => Ok(NiValue::Float(a - *b as f64)),
        _ => Err(NiRuntimeError::new(format!(
            "Cannot subtract {} from {}",
            right.type_name(),
            left.type_name()
        ))),
    }
}

pub fn ni_mul(left: &NiValue, right: &NiValue) -> NiResult<NiValue> {
    match (left, right) {
        (NiValue::Int(a), NiValue::Int(b)) => Ok(NiValue::Int(a.checked_mul(*b).ok_or_else(|| NiRuntimeError::new("Integer overflow"))?)),
        (NiValue::Float(a), NiValue::Float(b)) => Ok(NiValue::Float(a * b)),
        (NiValue::Int(a), NiValue::Float(b)) => Ok(NiValue::Float(*a as f64 * b)),
        (NiValue::Float(a), NiValue::Int(b)) => Ok(NiValue::Float(a * *b as f64)),
        (NiValue::String(s), NiValue::Int(n)) | (NiValue::Int(n), NiValue::String(s)) => {
            if *n <= 0 {
                Ok(NiValue::String(Rc::new(String::new())))
            } else {
                let total = s.len().saturating_mul(*n as usize);
                if total > 100 * 1024 * 1024 {
                    Err(NiRuntimeError::new(format!(
                        "String repetition too large ({} bytes)", total
                    )))
                } else {
                    Ok(NiValue::String(Rc::new(s.repeat(*n as usize))))
                }
            }
        }
        _ => Err(NiRuntimeError::new(format!(
            "Cannot multiply {} and {}",
            left.type_name(),
            right.type_name()
        ))),
    }
}

pub fn ni_div(left: &NiValue, right: &NiValue) -> NiResult<NiValue> {
    match (left, right) {
        (NiValue::Int(a), NiValue::Int(b)) => {
            if *b == 0 {
                return Err(NiRuntimeError::new("Division by zero"));
            }
            Ok(NiValue::Int(a.checked_div(*b).ok_or_else(|| NiRuntimeError::new("Integer overflow"))?))
        }
        (NiValue::Float(a), NiValue::Float(b)) => {
            if *b == 0.0 {
                return Err(NiRuntimeError::new("Division by zero"));
            }
            Ok(NiValue::Float(a / b))
        }
        (NiValue::Int(a), NiValue::Float(b)) => {
            if *b == 0.0 {
                return Err(NiRuntimeError::new("Division by zero"));
            }
            Ok(NiValue::Float(*a as f64 / b))
        }
        (NiValue::Float(a), NiValue::Int(b)) => {
            if *b == 0 {
                return Err(NiRuntimeError::new("Division by zero"));
            }
            Ok(NiValue::Float(a / *b as f64))
        }
        _ => Err(NiRuntimeError::new(format!(
            "Cannot divide {} by {}",
            left.type_name(),
            right.type_name()
        ))),
    }
}

pub fn ni_mod(left: &NiValue, right: &NiValue) -> NiResult<NiValue> {
    match (left, right) {
        (NiValue::Int(a), NiValue::Int(b)) => {
            if *b == 0 {
                return Err(NiRuntimeError::new("Modulo by zero"));
            }
            Ok(NiValue::Int(a.checked_rem(*b).ok_or_else(|| NiRuntimeError::new("Integer overflow"))?))
        }
        (NiValue::Float(a), NiValue::Float(b)) => {
            if *b == 0.0 {
                return Err(NiRuntimeError::new("Modulo by zero"));
            }
            Ok(NiValue::Float(a % b))
        }
        (NiValue::Int(a), NiValue::Float(b)) => {
            if *b == 0.0 {
                return Err(NiRuntimeError::new("Modulo by zero"));
            }
            Ok(NiValue::Float(*a as f64 % b))
        }
        (NiValue::Float(a), NiValue::Int(b)) => {
            if *b == 0 {
                return Err(NiRuntimeError::new("Modulo by zero"));
            }
            Ok(NiValue::Float(a % *b as f64))
        }
        _ => Err(NiRuntimeError::new(format!(
            "Cannot modulo {} by {}",
            left.type_name(),
            right.type_name()
        ))),
    }
}

pub fn ni_negate(value: &NiValue) -> NiResult<NiValue> {
    match value {
        NiValue::Int(n) => Ok(NiValue::Int(n.checked_neg().ok_or_else(|| NiRuntimeError::new("Integer overflow"))?)),
        NiValue::Float(f) => Ok(NiValue::Float(-f)),
        _ => Err(NiRuntimeError::new(format!(
            "Cannot negate {}",
            value.type_name()
        ))),
    }
}

pub fn ni_not(value: &NiValue) -> NiValue {
    NiValue::Bool(!value.is_truthy())
}

// Comparison operations

pub fn ni_eq(left: &NiValue, right: &NiValue) -> NiValue {
    NiValue::Bool(left == right)
}

pub fn ni_neq(left: &NiValue, right: &NiValue) -> NiValue {
    NiValue::Bool(left != right)
}

pub fn ni_lt(left: &NiValue, right: &NiValue) -> NiResult<NiValue> {
    match (left, right) {
        (NiValue::Int(a), NiValue::Int(b)) => Ok(NiValue::Bool(a < b)),
        (NiValue::Float(a), NiValue::Float(b)) => Ok(NiValue::Bool(a < b)),
        (NiValue::Int(a), NiValue::Float(b)) => Ok(NiValue::Bool((*a as f64) < *b)),
        (NiValue::Float(a), NiValue::Int(b)) => Ok(NiValue::Bool(*a < (*b as f64))),
        (NiValue::String(a), NiValue::String(b)) => Ok(NiValue::Bool(a < b)),
        _ => Err(NiRuntimeError::new(format!(
            "Cannot compare {} < {}",
            left.type_name(),
            right.type_name()
        ))),
    }
}

pub fn ni_gt(left: &NiValue, right: &NiValue) -> NiResult<NiValue> {
    match (left, right) {
        (NiValue::Int(a), NiValue::Int(b)) => Ok(NiValue::Bool(a > b)),
        (NiValue::Float(a), NiValue::Float(b)) => Ok(NiValue::Bool(a > b)),
        (NiValue::Int(a), NiValue::Float(b)) => Ok(NiValue::Bool((*a as f64) > *b)),
        (NiValue::Float(a), NiValue::Int(b)) => Ok(NiValue::Bool(*a > (*b as f64))),
        (NiValue::String(a), NiValue::String(b)) => Ok(NiValue::Bool(a > b)),
        _ => Err(NiRuntimeError::new(format!(
            "Cannot compare {} > {}",
            left.type_name(),
            right.type_name()
        ))),
    }
}

pub fn ni_lte(left: &NiValue, right: &NiValue) -> NiResult<NiValue> {
    match (left, right) {
        (NiValue::Int(a), NiValue::Int(b)) => Ok(NiValue::Bool(a <= b)),
        (NiValue::Float(a), NiValue::Float(b)) => Ok(NiValue::Bool(a <= b)),
        (NiValue::Int(a), NiValue::Float(b)) => Ok(NiValue::Bool((*a as f64) <= *b)),
        (NiValue::Float(a), NiValue::Int(b)) => Ok(NiValue::Bool(*a <= (*b as f64))),
        (NiValue::String(a), NiValue::String(b)) => Ok(NiValue::Bool(a <= b)),
        _ => Err(NiRuntimeError::new(format!(
            "Cannot compare {} <= {}",
            left.type_name(),
            right.type_name()
        ))),
    }
}

pub fn ni_gte(left: &NiValue, right: &NiValue) -> NiResult<NiValue> {
    match (left, right) {
        (NiValue::Int(a), NiValue::Int(b)) => Ok(NiValue::Bool(a >= b)),
        (NiValue::Float(a), NiValue::Float(b)) => Ok(NiValue::Bool(a >= b)),
        (NiValue::Int(a), NiValue::Float(b)) => Ok(NiValue::Bool((*a as f64) >= *b)),
        (NiValue::Float(a), NiValue::Int(b)) => Ok(NiValue::Bool(*a >= (*b as f64))),
        (NiValue::String(a), NiValue::String(b)) => Ok(NiValue::Bool(a >= b)),
        _ => Err(NiRuntimeError::new(format!(
            "Cannot compare {} >= {}",
            left.type_name(),
            right.type_name()
        ))),
    }
}

pub fn ni_is_truthy(value: &NiValue) -> bool {
    value.is_truthy()
}

// Type checking

pub fn ni_is(value: &NiValue, type_name: &str) -> NiValue {
    let result = match type_name {
        "Int" => matches!(value, NiValue::Int(_)),
        "Float" => matches!(value, NiValue::Float(_)),
        "Bool" => matches!(value, NiValue::Bool(_)),
        "String" => matches!(value, NiValue::String(_)),
        "List" => matches!(value, NiValue::List(_)),
        "Map" => matches!(value, NiValue::Map(_)),
        "None" => matches!(value, NiValue::None),
        "Function" => matches!(value, NiValue::Function(_)),
        other => {
            if let NiValue::Instance(inst) = value {
                inst.borrow().class_name == other
            } else {
                false
            }
        }
    };
    NiValue::Bool(result)
}

pub fn ni_in(needle: &NiValue, haystack: &NiValue) -> NiResult<NiValue> {
    match haystack {
        NiValue::List(list) => {
            let found = list.borrow().iter().any(|v| v == needle);
            Ok(NiValue::Bool(found))
        }
        NiValue::Map(map) => {
            let found = map.borrow().iter().any(|(k, _)| k == needle);
            Ok(NiValue::Bool(found))
        }
        NiValue::String(s) => {
            if let NiValue::String(substr) = needle {
                Ok(NiValue::Bool(s.contains(substr.as_str())))
            } else {
                Err(NiRuntimeError::new(
                    "'in' operator requires String needle for String haystack",
                ))
            }
        }
        NiValue::Range(range) => {
            if let NiValue::Int(n) = needle {
                let in_range = if range.inclusive {
                    *n >= range.start && *n <= range.end
                } else {
                    *n >= range.start && *n < range.end
                };
                Ok(NiValue::Bool(in_range))
            } else {
                Err(NiRuntimeError::new("'in' operator requires Int for Range"))
            }
        }
        _ => Err(NiRuntimeError::new(format!(
            "Cannot use 'in' operator with {}",
            haystack.type_name()
        ))),
    }
}

// Iterator support

pub enum NiIterator {
    Range {
        current: i64,
        end: i64,
        inclusive: bool,
        step: i64,
    },
    List {
        list: Rc<RefCell<Vec<NiValue>>>,
        index: usize,
    },
    Map {
        map: Rc<RefCell<Vec<(NiValue, NiValue)>>>,
        index: usize,
    },
    String {
        string: Rc<String>,
        index: usize,
    },
}

pub fn ni_get_iterator(value: &NiValue) -> NiResult<NiIterator> {
    match value {
        NiValue::Range(range) => Ok(NiIterator::Range {
            current: range.start,
            end: range.end,
            inclusive: range.inclusive,
            step: range.step,
        }),
        NiValue::List(list) => Ok(NiIterator::List {
            list: list.clone(),
            index: 0,
        }),
        NiValue::Map(map) => Ok(NiIterator::Map {
            map: map.clone(),
            index: 0,
        }),
        NiValue::String(s) => Ok(NiIterator::String {
            string: s.clone(),
            index: 0,
        }),
        _ => Err(NiRuntimeError::new(format!(
            "Cannot iterate over {}",
            value.type_name()
        ))),
    }
}

pub fn ni_iterator_next(iter: &mut NiIterator) -> NiResult<Option<NiValue>> {
    match iter {
        NiIterator::Range {
            current,
            end,
            inclusive,
            step,
        } => {
            let in_range = if *step > 0 {
                if *inclusive {
                    *current <= *end
                } else {
                    *current < *end
                }
            } else {
                // negative step: iterate downward
                if *inclusive {
                    *current >= *end
                } else {
                    *current > *end
                }
            };
            if in_range {
                let val = *current;
                *current += *step;
                Ok(Some(NiValue::Int(val)))
            } else {
                Ok(None)
            }
        }
        NiIterator::List { list, index } => {
            let list = list.borrow();
            if *index < list.len() {
                let val = list[*index].clone();
                *index += 1;
                Ok(Some(val))
            } else {
                Ok(None)
            }
        }
        NiIterator::Map { map, index } => {
            let map = map.borrow();
            if *index < map.len() {
                let (key, _) = &map[*index];
                let val = key.clone();
                *index += 1;
                Ok(Some(val))
            } else {
                Ok(None)
            }
        }
        NiIterator::String { string, index } => {
            let chars: Vec<char> = string.chars().collect();
            if *index < chars.len() {
                let val = NiValue::String(Rc::new(chars[*index].to_string()));
                *index += 1;
                Ok(Some(val))
            } else {
                Ok(None)
            }
        }
    }
}

/// For `for key, value in map:` -- returns (key, value) pairs
pub fn ni_iterator_next_pair(iter: &mut NiIterator) -> NiResult<Option<(NiValue, NiValue)>> {
    match iter {
        NiIterator::Map { map, index } => {
            let map = map.borrow();
            if *index < map.len() {
                let (key, val) = &map[*index];
                let pair = (key.clone(), val.clone());
                *index += 1;
                Ok(Some(pair))
            } else {
                Ok(None)
            }
        }
        NiIterator::List { list, index } => {
            let list = list.borrow();
            if *index < list.len() {
                let pair = (NiValue::Int(*index as i64), list[*index].clone());
                *index += 1;
                Ok(Some(pair))
            } else {
                Ok(None)
            }
        }
        _ => {
            // Fall back to single-value iteration
            if let Some(val) = ni_iterator_next(iter)? {
                Ok(Some((NiValue::Int(0), val)))
            } else {
                Ok(None)
            }
        }
    }
}

// Index operations

pub fn ni_get_index(collection: &NiValue, index: &NiValue) -> NiResult<NiValue> {
    match (collection, index) {
        (NiValue::List(list), NiValue::Int(i)) => {
            let list = list.borrow();
            let idx = if *i < 0 {
                (list.len() as i64 + i) as usize
            } else {
                *i as usize
            };
            list.get(idx).cloned().ok_or_else(|| {
                NiRuntimeError::new(format!("Index {} out of bounds (len {})", i, list.len()))
            })
        }
        (NiValue::Map(map), key) => {
            let map = map.borrow();
            for (k, v) in map.iter() {
                if k == key {
                    return Ok(v.clone());
                }
            }
            Err(NiRuntimeError::new("Key not found in map".to_string()))
        }
        (NiValue::String(s), NiValue::Int(i)) => {
            let chars: Vec<char> = s.chars().collect();
            let idx = if *i < 0 {
                (chars.len() as i64 + i) as usize
            } else {
                *i as usize
            };
            chars
                .get(idx)
                .map(|c| NiValue::String(Rc::new(c.to_string())))
                .ok_or_else(|| NiRuntimeError::new(format!("Index {} out of bounds", i)))
        }
        _ => Err(NiRuntimeError::new(format!(
            "Cannot index {} with {}",
            collection.type_name(),
            index.type_name()
        ))),
    }
}

pub fn ni_set_index(collection: &NiValue, index: &NiValue, value: NiValue) -> NiResult<()> {
    match (collection, index) {
        (NiValue::List(list), NiValue::Int(i)) => {
            let mut list = list.borrow_mut();
            let idx = if *i < 0 {
                (list.len() as i64 + i) as usize
            } else {
                *i as usize
            };
            if idx < list.len() {
                list[idx] = value;
                Ok(())
            } else {
                Err(NiRuntimeError::new(format!(
                    "Index {} out of bounds (len {})",
                    i,
                    list.len()
                )))
            }
        }
        (NiValue::Map(map), key) => {
            let mut map = map.borrow_mut();
            for (k, v) in map.iter_mut() {
                if k == key {
                    *v = value;
                    return Ok(());
                }
            }
            map.push((key.clone(), value));
            Ok(())
        }
        _ => Err(NiRuntimeError::new(format!(
            "Cannot set index on {}",
            collection.type_name()
        ))),
    }
}

// Function call helper

pub fn ni_call(
    vm: &mut dyn crate::vm_context::NiVm,
    callee: &NiValue,
    args: &[NiValue],
) -> NiResult<NiValue> {
    match callee {
        NiValue::Function(func_ref) => (func_ref.func)(vm, args),
        NiValue::Class(class_def) => {
            // Instantiate
            let instance = crate::class::NiInstance::new(class_def.clone());
            let instance_val = NiValue::Instance(Rc::new(RefCell::new(instance)));
            // Call init if present
            if let Some(init) = class_def.find_method("init") {
                init(vm, &instance_val, args)?;
            }
            Ok(instance_val)
        }
        _ => Err(NiRuntimeError::new(format!(
            "Cannot call {}",
            callee.type_name()
        ))),
    }
}
