use crate::gc::{GcHeap, GcRef};
use crate::intern::InternTable;
use crate::object::NiObject;
use crate::value::{self, Value};

pub fn call_method(
    heap: &mut GcHeap,
    receiver_ref: GcRef,
    method: &str,
    args: &[Value],
    interner: &InternTable,
) -> Result<Option<Value>, String> {
    let obj = heap.get(receiver_ref).ok_or("Invalid object reference")?;
    match obj {
        NiObject::String(_) | NiObject::InternedString(_) => {
            call_string_method(heap, receiver_ref, method, args, interner)
        }
        NiObject::List(_) => call_list_method(heap, receiver_ref, method, args, interner),
        NiObject::Bytes(_) => call_bytes_method(heap, receiver_ref, method, args),
        NiObject::Map(_) => call_map_method(heap, receiver_ref, method, args, interner),
        NiObject::Range(_) => call_range_method(heap, receiver_ref, method),
        _ => Ok(None), // Not a stdlib method, fall through to class method dispatch
    }
}

fn call_string_method(
    heap: &mut GcHeap,
    receiver: GcRef,
    method: &str,
    args: &[Value],
    interner: &InternTable,
) -> Result<Option<Value>, String> {
    let s = heap
        .get(receiver)
        .unwrap()
        .as_string_with_intern(interner)
        .unwrap()
        .to_string();
    match method {
        "upper" => {
            let r = heap.alloc(NiObject::String(s.to_uppercase()));
            Ok(Some(Value::Object(r)))
        }
        "lower" => {
            let r = heap.alloc(NiObject::String(s.to_lowercase()));
            Ok(Some(Value::Object(r)))
        }
        "trim" => {
            let r = heap.alloc(NiObject::String(s.trim().to_string()));
            Ok(Some(Value::Object(r)))
        }
        "split" => {
            let sep = get_string_arg(heap, args, 0, interner)?;
            let limit = args.get(1).and_then(|v| v.as_int());
            let parts: Vec<Value> = if let Some(n) = limit {
                s.splitn(n.max(1) as usize, &sep)
                    .map(|p| {
                        let r = heap.alloc(NiObject::String(p.to_string()));
                        Value::Object(r)
                    })
                    .collect()
            } else {
                s.split(&sep)
                    .take(10_000)
                    .map(|p| {
                        let r = heap.alloc(NiObject::String(p.to_string()));
                        Value::Object(r)
                    })
                    .collect()
            };
            let r = heap.alloc(NiObject::List(parts));
            Ok(Some(Value::Object(r)))
        }
        "index_of" => {
            let sub = get_string_arg(heap, args, 0, interner)?;
            let idx = s.find(&sub).map(|byte_idx| s[..byte_idx].chars().count() as i64).unwrap_or(-1);
            Ok(Some(Value::Int(idx)))
        }
        "contains" => {
            let sub = get_string_arg(heap, args, 0, interner)?;
            Ok(Some(Value::Bool(s.contains(&sub))))
        }
        "starts_with" => {
            let prefix = get_string_arg(heap, args, 0, interner)?;
            Ok(Some(Value::Bool(s.starts_with(&prefix))))
        }
        "ends_with" => {
            let suffix = get_string_arg(heap, args, 0, interner)?;
            Ok(Some(Value::Bool(s.ends_with(&suffix))))
        }
        "replace" => {
            let from = get_string_arg(heap, args, 0, interner)?;
            let to = get_string_arg(heap, args, 1, interner)?;
            let r = heap.alloc(NiObject::String(s.replace(&from, &to)));
            Ok(Some(Value::Object(r)))
        }
        "slice" => {
            let start = args.first().and_then(|v| v.as_int()).unwrap_or(0) as usize;
            let end = args
                .get(1)
                .and_then(|v| v.as_int())
                .unwrap_or(s.chars().count() as i64) as usize;
            let sliced: String = s
                .chars()
                .skip(start)
                .take(end.saturating_sub(start))
                .collect();
            let r = heap.alloc(NiObject::String(sliced));
            Ok(Some(Value::Object(r)))
        }
        "char_at" => {
            let idx = args.first().and_then(|v| v.as_int()).unwrap_or(0) as usize;
            let ch = s
                .chars()
                .nth(idx)
                .map(|c| c.to_string())
                .unwrap_or_default();
            let r = heap.alloc(NiObject::String(ch));
            Ok(Some(Value::Object(r)))
        }
        "to_int" => {
            let n = s
                .parse::<i64>()
                .map_err(|_| format!("Cannot convert '{}' to int", s))?;
            Ok(Some(Value::Int(n)))
        }
        "to_float" => {
            let n = s
                .parse::<f64>()
                .map_err(|_| format!("Cannot convert '{}' to float", s))?;
            Ok(Some(Value::Float(n)))
        }
        _ => Ok(None),
    }
}

fn call_list_method(
    heap: &mut GcHeap,
    receiver: GcRef,
    method: &str,
    args: &[Value],
    interner: &InternTable,
) -> Result<Option<Value>, String> {
    match method {
        "add" => {
            let val = args.first().cloned().unwrap_or(Value::None);
            let list = heap.get_mut(receiver).unwrap().as_list_mut().unwrap();
            list.push(val);
            Ok(Some(Value::None))
        }
        "insert" => {
            let idx = args.first().and_then(|v| v.as_int()).unwrap_or(0) as usize;
            let val = args.get(1).cloned().unwrap_or(Value::None);
            let list = heap.get_mut(receiver).unwrap().as_list_mut().unwrap();
            if idx <= list.len() {
                list.insert(idx, val);
            }
            Ok(Some(Value::None))
        }
        "remove" => {
            let val = args.first().cloned().unwrap_or(Value::None);
            let list = heap.get_mut(receiver).unwrap().as_list_mut().unwrap();
            if let Some(pos) = list.iter().position(|v| *v == val) {
                list.remove(pos);
            }
            Ok(Some(Value::None))
        }
        "remove_at" => {
            let idx = args.first().and_then(|v| v.as_int()).unwrap_or(0) as usize;
            let list = heap.get_mut(receiver).unwrap().as_list_mut().unwrap();
            if idx < list.len() {
                let v = list.remove(idx);
                return Ok(Some(v));
            }
            Ok(Some(Value::None))
        }
        "pop" => {
            let list = heap.get_mut(receiver).unwrap().as_list_mut().unwrap();
            let v = list.pop().unwrap_or(Value::None);
            Ok(Some(v))
        }
        "contains" => {
            let val = args.first().cloned().unwrap_or(Value::None);
            let list = heap.get(receiver).unwrap().as_list().unwrap();
            Ok(Some(Value::Bool(list.contains(&val))))
        }
        "index_of" => {
            let val = args.first().cloned().unwrap_or(Value::None);
            let list = heap.get(receiver).unwrap().as_list().unwrap();
            let idx = list
                .iter()
                .position(|v| *v == val)
                .map(|i| i as i64)
                .unwrap_or(-1);
            Ok(Some(Value::Int(idx)))
        }
        "sort" => {
            let list = heap.get_mut(receiver).unwrap().as_list_mut().unwrap();
            list.sort_by(|a, b| {
                let fa = a.to_number().unwrap_or(0.0);
                let fb = b.to_number().unwrap_or(0.0);
                fa.partial_cmp(&fb).unwrap_or(std::cmp::Ordering::Equal)
            });
            Ok(Some(Value::None))
        }
        "reverse" => {
            let list = heap.get_mut(receiver).unwrap().as_list_mut().unwrap();
            list.reverse();
            Ok(Some(Value::None))
        }
        "copy" => {
            let list = heap.get(receiver).unwrap().as_list().unwrap().clone();
            let r = heap.alloc(NiObject::List(list));
            Ok(Some(Value::Object(r)))
        }
        "join" => {
            let sep = match args.first() {
                Some(Value::Object(r)) => heap
                    .get(*r)
                    .and_then(|o| o.as_string_with_intern(interner))
                    .map(|s| s.to_string())
                    .unwrap_or_default(),
                _ => String::new(),
            };
            let list = heap.get(receiver).unwrap().as_list().unwrap();
            let parts: Vec<String> = list
                .iter()
                .map(|v| crate::native::value_to_display_string(v, heap, interner))
                .collect();
            let joined = parts.join(&sep);
            let r = heap.alloc(NiObject::String(joined));
            Ok(Some(Value::Object(r)))
        }
        "filter" | "map" | "any" | "all" => {
            // These require calling closures -- handled by VM
            Ok(None)
        }
        _ => Ok(None),
    }
}

fn call_bytes_method(
    heap: &mut GcHeap,
    receiver: GcRef,
    method: &str,
    args: &[Value],
) -> Result<Option<Value>, String> {
    match method {
        "slice" => {
            let bytes = heap.get(receiver).unwrap().as_bytes().unwrap();
            let len = bytes.len() as i64;
            let start = args.first().and_then(|v| v.as_int()).unwrap_or(0);
            let end = args.get(1).and_then(|v| v.as_int()).unwrap_or(len);
            let start = start.max(0).min(len) as usize;
            let end = end.max(0).min(len) as usize;
            let end = end.max(start);
            let sliced = bytes[start..end].to_vec();
            let r = heap.alloc(NiObject::Bytes(sliced));
            Ok(Some(Value::Object(r)))
        }
        "add" => {
            let byte_val = args
                .first()
                .and_then(|v| v.as_int())
                .ok_or("add() requires an integer argument")?;
            if !(0..=255).contains(&byte_val) {
                return Err(format!("Byte value {} out of range (0-255)", byte_val));
            }
            let bytes = heap.get_mut(receiver).unwrap().as_bytes_mut().unwrap();
            bytes.push(byte_val as u8);
            Ok(Some(Value::None))
        }
        "pop" => {
            let bytes = heap.get_mut(receiver).unwrap().as_bytes_mut().unwrap();
            match bytes.pop() {
                Some(b) => Ok(Some(Value::Int(b as i64))),
                None => Ok(Some(Value::None)),
            }
        }
        "to_list" => {
            let bytes = heap.get(receiver).unwrap().as_bytes().unwrap();
            let items: Vec<Value> = bytes.iter().map(|b| Value::Int(*b as i64)).collect();
            let r = heap.alloc(NiObject::List(items));
            Ok(Some(Value::Object(r)))
        }
        "contains" => {
            let byte_val = args
                .first()
                .and_then(|v| v.as_int())
                .ok_or("contains() requires an integer argument")?;
            if !(0..=255).contains(&byte_val) {
                return Ok(Some(Value::Bool(false)));
            }
            let bytes = heap.get(receiver).unwrap().as_bytes().unwrap();
            Ok(Some(Value::Bool(bytes.contains(&(byte_val as u8)))))
        }
        _ => Ok(None),
    }
}

fn call_map_method(
    heap: &mut GcHeap,
    receiver: GcRef,
    method: &str,
    args: &[Value],
    interner: &InternTable,
) -> Result<Option<Value>, String> {
    match method {
        "keys" => {
            let map = heap.get(receiver).unwrap().as_map().unwrap();
            let keys: Vec<Value> = map.iter().map(|(k, _)| k.clone()).collect();
            let r = heap.alloc(NiObject::List(keys));
            Ok(Some(Value::Object(r)))
        }
        "values" => {
            let map = heap.get(receiver).unwrap().as_map().unwrap();
            let vals: Vec<Value> = map.iter().map(|(_, v)| v.clone()).collect();
            let r = heap.alloc(NiObject::List(vals));
            Ok(Some(Value::Object(r)))
        }
        "contains_key" => {
            let key = args.first().cloned().unwrap_or(Value::None);
            let map = heap.get(receiver).unwrap().as_map().unwrap();
            let found = map
                .iter()
                .any(|(k, _)| value::values_equal(k, &key, heap, interner));
            Ok(Some(Value::Bool(found)))
        }
        "get" => {
            let key = args.first().cloned().unwrap_or(Value::None);
            let default = args.get(1).cloned().unwrap_or(Value::None);
            let map = heap.get(receiver).unwrap().as_map().unwrap();
            let val = map
                .iter()
                .find(|(k, _)| value::values_equal(k, &key, heap, interner))
                .map(|(_, v)| v.clone())
                .unwrap_or(default);
            Ok(Some(val))
        }
        "remove" => {
            let key = args.first().cloned().unwrap_or(Value::None);
            let pos = {
                let map = heap.get(receiver).unwrap().as_map().unwrap();
                map.iter()
                    .position(|(k, _)| value::values_equal(k, &key, heap, interner))
            };
            if let Some(pos) = pos {
                heap.get_mut(receiver)
                    .unwrap()
                    .as_map_mut()
                    .unwrap()
                    .remove(pos);
            }
            Ok(Some(Value::None))
        }
        "copy" => {
            let map = heap.get(receiver).unwrap().as_map().unwrap().clone();
            let r = heap.alloc(NiObject::Map(map));
            Ok(Some(Value::Object(r)))
        }
        _ => Ok(None),
    }
}

fn call_range_method(
    _heap: &mut GcHeap,
    _receiver: GcRef,
    _method: &str,
) -> Result<Option<Value>, String> {
    Ok(None)
}

fn get_string_arg(
    heap: &GcHeap,
    args: &[Value],
    idx: usize,
    interner: &InternTable,
) -> Result<String, String> {
    match args.get(idx) {
        Some(Value::Object(r)) => heap
            .get(*r)
            .and_then(|o| o.as_string_with_intern(interner))
            .map(|s| s.to_string())
            .ok_or_else(|| format!("Expected string argument at position {}", idx)),
        _ => Err(format!("Expected string argument at position {}", idx)),
    }
}

// Get a property from a string/list/map (like .length)
pub fn get_property(
    heap: &mut GcHeap,
    receiver_ref: GcRef,
    property: &str,
    interner: &InternTable,
) -> Result<Option<Value>, String> {
    let obj = heap.get(receiver_ref).ok_or("Invalid object reference")?;
    match obj {
        NiObject::String(s) => match property {
            "length" => Ok(Some(Value::Int(s.chars().count() as i64))),
            _ => Ok(None),
        },
        NiObject::InternedString(id) => match property {
            "length" => Ok(Some(Value::Int(interner.resolve(*id).chars().count() as i64))),
            _ => Ok(None),
        },
        NiObject::List(l) => match property {
            "length" => Ok(Some(Value::Int(l.len() as i64))),
            _ => Ok(None),
        },
        NiObject::Bytes(b) => match property {
            "length" => Ok(Some(Value::Int(b.len() as i64))),
            _ => Ok(None),
        },
        NiObject::Map(m) => match property {
            "length" => Ok(Some(Value::Int(m.len() as i64))),
            _ => Ok(None),
        },
        _ => Ok(None),
    }
}
