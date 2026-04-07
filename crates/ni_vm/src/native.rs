use std::cell::RefCell;

use crate::gc::GcHeap;
use crate::intern::InternTable;
use crate::object::{NativeFn, NativeResult, NiObject};
use crate::value::Value;

// Thread-local xorshift64 RNG for the random module
thread_local! {
    static RNG_STATE: RefCell<u64> = RefCell::new({
        use std::time::{SystemTime, UNIX_EPOCH};
        let seed = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_nanos() as u64)
            .unwrap_or(0xDEAD_BEEF_CAFE_BABE);
        if seed == 0 { 1 } else { seed }
    });
}

fn xorshift64() -> u64 {
    RNG_STATE.with(|state| {
        let mut s = state.borrow_mut();
        *s ^= *s << 13;
        *s ^= *s >> 7;
        *s ^= *s << 17;
        *s
    })
}

fn random_f64() -> f64 {
    // Generate a float in [0, 1)
    (xorshift64() >> 11) as f64 / ((1u64 << 53) as f64)
}

pub fn all_natives() -> Vec<NativeFn> {
    vec![
        NativeFn {
            name: "print".into(),
            arity: -1,
            function: native_print,
        },
        NativeFn {
            name: "type_of".into(),
            arity: 1,
            function: native_type_of,
        },
        NativeFn {
            name: "type".into(),
            arity: 1,
            function: native_type_of,
        },
        NativeFn {
            name: "len".into(),
            arity: 1,
            function: native_len,
        },
        NativeFn {
            name: "clock".into(),
            arity: 0,
            function: native_clock,
        },
        NativeFn {
            name: "to_string".into(),
            arity: 1,
            function: native_to_string,
        },
        NativeFn {
            name: "to_int".into(),
            arity: 1,
            function: native_to_int,
        },
        NativeFn {
            name: "to_float".into(),
            arity: 1,
            function: native_to_float,
        },
        NativeFn {
            name: "to_bool".into(),
            arity: 1,
            function: native_to_bool,
        },
        NativeFn {
            name: "abs".into(),
            arity: 1,
            function: native_abs,
        },
        NativeFn {
            name: "min".into(),
            arity: 2,
            function: native_min,
        },
        NativeFn {
            name: "max".into(),
            arity: 2,
            function: native_max,
        },
        NativeFn {
            name: "clamp".into(),
            arity: 3,
            function: native_clamp,
        },
        NativeFn {
            name: "sqrt".into(),
            arity: 1,
            function: native_sqrt,
        },
        NativeFn {
            name: "floor".into(),
            arity: 1,
            function: native_floor,
        },
        NativeFn {
            name: "ceil".into(),
            arity: 1,
            function: native_ceil,
        },
        NativeFn {
            name: "round".into(),
            arity: 1,
            function: native_round,
        },
        NativeFn {
            name: "sin".into(),
            arity: 1,
            function: native_sin,
        },
        NativeFn {
            name: "cos".into(),
            arity: 1,
            function: native_cos,
        },
        NativeFn {
            name: "enumerate".into(),
            arity: 1,
            function: native_enumerate,
        },
        NativeFn {
            name: "range".into(),
            arity: -1,
            function: native_range,
        },
        NativeFn {
            name: "log".into(),
            arity: -1,
            function: native_log,
        },
        NativeFn {
            name: "log_warning".into(),
            arity: -1,
            function: native_log,
        },
        NativeFn {
            name: "log_error".into(),
            arity: -1,
            function: native_log,
        },
        NativeFn {
            name: "Bytes".into(),
            arity: -1,
            function: native_bytes,
        },
    ]
}

fn native_print(args: &[Value], heap: &mut GcHeap, interner: &InternTable) -> NativeResult {
    {
        let parts: Vec<String> = args
            .iter()
            .map(|v| value_to_display_string(v, heap, interner))
            .collect();
        println!("{}", parts.join(" "));
        Ok(Value::None)
    }
    .into()
}

fn native_type_of(args: &[Value], heap: &mut GcHeap, _interner: &InternTable) -> NativeResult {
    {
        let name = match &args[0] {
            Value::Int(_) => "int",
            Value::Float(_) => "float",
            Value::Bool(_) => "bool",
            Value::None => "none",
            Value::Object(r) => {
                if let Some(obj) = heap.get(*r) {
                    obj.type_name()
                } else {
                    "object"
                }
            }
        };
        let s = heap.alloc(NiObject::String(name.to_string()));
        Ok(Value::Object(s))
    }
    .into()
}

fn native_len(args: &[Value], heap: &mut GcHeap, interner: &InternTable) -> NativeResult {
    (match &args[0] {
        Value::Object(r) => {
            if let Some(obj) = heap.get(*r) {
                match obj {
                    NiObject::String(s) => Ok(Value::Int(s.chars().count() as i64)),
                    NiObject::InternedString(id) => {
                        Ok(Value::Int(interner.resolve(*id).chars().count() as i64))
                    }
                    NiObject::List(l) => Ok(Value::Int(l.len() as i64)),
                    NiObject::Bytes(b) => Ok(Value::Int(b.len() as i64)),
                    NiObject::Map(m) => Ok(Value::Int(m.len() as i64)),
                    _ => Err("len() not supported for this type".into()),
                }
            } else {
                Err("Invalid reference".into())
            }
        }
        _ => Err("len() requires a string, list, or map".into()),
    })
    .into()
}

fn native_clock(_args: &[Value], _heap: &mut GcHeap, _interner: &InternTable) -> NativeResult {
    (|| -> Result<Value, String> {
        use std::time::{SystemTime, UNIX_EPOCH};
        let secs = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|e| e.to_string())?
            .as_secs_f64();
        Ok(Value::Float(secs))
    })()
    .into()
}

fn native_to_string(args: &[Value], heap: &mut GcHeap, interner: &InternTable) -> NativeResult {
    {
        let s = value_to_display_string(&args[0], heap, interner);
        let r = heap.alloc(NiObject::String(s));
        Ok(Value::Object(r))
    }
    .into()
}

fn native_to_int(args: &[Value], heap: &mut GcHeap, interner: &InternTable) -> NativeResult {
    (match &args[0] {
        Value::Int(n) => Ok(Value::Int(*n)),
        Value::Float(f) => Ok(Value::Int(*f as i64)),
        Value::Bool(b) => Ok(Value::Int(if *b { 1 } else { 0 })),
        Value::Object(r) => {
            if let Some(obj) = heap.get(*r) {
                let s = obj.as_string_with_intern(interner);
                if let Some(s) = s {
                    s.parse::<i64>()
                        .map(Value::Int)
                        .map_err(|_| format!("Cannot convert '{}' to int", s))
                } else {
                    Err("Cannot convert to int".into())
                }
            } else {
                Err("Cannot convert to int".into())
            }
        }
        _ => Err("Cannot convert to int".into()),
    })
    .into()
}

fn native_to_float(args: &[Value], heap: &mut GcHeap, interner: &InternTable) -> NativeResult {
    (match &args[0] {
        Value::Int(n) => Ok(Value::Float(*n as f64)),
        Value::Float(f) => Ok(Value::Float(*f)),
        Value::Bool(b) => Ok(Value::Float(if *b { 1.0 } else { 0.0 })),
        Value::Object(r) => {
            if let Some(obj) = heap.get(*r) {
                let s = obj.as_string_with_intern(interner);
                if let Some(s) = s {
                    s.parse::<f64>()
                        .map(Value::Float)
                        .map_err(|_| format!("Cannot convert '{}' to float", s))
                } else {
                    Err("Cannot convert to float".into())
                }
            } else {
                Err("Cannot convert to float".into())
            }
        }
        _ => Err("Cannot convert to float".into()),
    })
    .into()
}

fn native_to_bool(args: &[Value], heap: &mut GcHeap, _interner: &InternTable) -> NativeResult {
    {
        let is_falsy = match &args[0] {
            Value::Bool(b) => !b,
            Value::None => true,
            Value::Int(0) => true,
            Value::Float(f) => *f == 0.0,
            Value::Object(r) => heap.get(*r).map(|o| o.is_falsy()).unwrap_or(true),
            _ => false,
        };
        Ok(Value::Bool(!is_falsy))
    }
    .into()
}

fn native_abs(args: &[Value], _heap: &mut GcHeap, _interner: &InternTable) -> NativeResult {
    (match &args[0] {
        Value::Int(n) => n.checked_abs()
            .map(Value::Int)
            .ok_or_else(|| "Integer overflow in abs()".to_string()),
        Value::Float(f) => Ok(Value::Float(f.abs())),
        _ => Err("abs() requires a number".into()),
    })
    .into()
}

fn native_min(args: &[Value], _heap: &mut GcHeap, _interner: &InternTable) -> NativeResult {
    (match (&args[0], &args[1]) {
        (Value::Int(a), Value::Int(b)) => Ok(Value::Int(*a.min(b))),
        (Value::Float(a), Value::Float(b)) => Ok(Value::Float(a.min(*b))),
        (Value::Int(a), Value::Float(b)) => Ok(Value::Float((*a as f64).min(*b))),
        (Value::Float(a), Value::Int(b)) => Ok(Value::Float(a.min(*b as f64))),
        _ => Err("min() requires numbers".into()),
    })
    .into()
}

fn native_max(args: &[Value], _heap: &mut GcHeap, _interner: &InternTable) -> NativeResult {
    (match (&args[0], &args[1]) {
        (Value::Int(a), Value::Int(b)) => Ok(Value::Int(*a.max(b))),
        (Value::Float(a), Value::Float(b)) => Ok(Value::Float(a.max(*b))),
        (Value::Int(a), Value::Float(b)) => Ok(Value::Float((*a as f64).max(*b))),
        (Value::Float(a), Value::Int(b)) => Ok(Value::Float(a.max(*b as f64))),
        _ => Err("max() requires numbers".into()),
    })
    .into()
}

fn native_clamp(args: &[Value], _heap: &mut GcHeap, _interner: &InternTable) -> NativeResult {
    (|| -> Result<Value, String> {
        let val = args[0].to_number().ok_or("clamp() requires numbers")?;
        let lo = args[1].to_number().ok_or("clamp() requires numbers")?;
        let hi = args[2].to_number().ok_or("clamp() requires numbers")?;
        let result = val.max(lo).min(hi);
        if matches!(
            (&args[0], &args[1], &args[2]),
            (Value::Int(_), Value::Int(_), Value::Int(_))
        ) {
            Ok(Value::Int(result as i64))
        } else {
            Ok(Value::Float(result))
        }
    })()
    .into()
}

fn native_sqrt(args: &[Value], _heap: &mut GcHeap, _interner: &InternTable) -> NativeResult {
    (|| -> Result<Value, String> {
        let n = args[0].to_number().ok_or("sqrt() requires a number")?;
        Ok(Value::Float(n.sqrt()))
    })()
    .into()
}

fn native_floor(args: &[Value], _heap: &mut GcHeap, _interner: &InternTable) -> NativeResult {
    (|| -> Result<Value, String> {
        let n = args[0].to_number().ok_or("floor() requires a number")?;
        Ok(Value::Int(n.floor() as i64))
    })()
    .into()
}

fn native_ceil(args: &[Value], _heap: &mut GcHeap, _interner: &InternTable) -> NativeResult {
    (|| -> Result<Value, String> {
        let n = args[0].to_number().ok_or("ceil() requires a number")?;
        Ok(Value::Int(n.ceil() as i64))
    })()
    .into()
}

fn native_round(args: &[Value], _heap: &mut GcHeap, _interner: &InternTable) -> NativeResult {
    (|| -> Result<Value, String> {
        let n = args[0].to_number().ok_or("round() requires a number")?;
        Ok(Value::Int(n.round() as i64))
    })()
    .into()
}

fn native_sin(args: &[Value], _heap: &mut GcHeap, _interner: &InternTable) -> NativeResult {
    (|| -> Result<Value, String> {
        let n = args[0].to_number().ok_or("sin() requires a number")?;
        Ok(Value::Float(n.sin()))
    })()
    .into()
}

fn native_cos(args: &[Value], _heap: &mut GcHeap, _interner: &InternTable) -> NativeResult {
    (|| -> Result<Value, String> {
        let n = args[0].to_number().ok_or("cos() requires a number")?;
        Ok(Value::Float(n.cos()))
    })()
    .into()
}

fn native_enumerate(args: &[Value], _heap: &mut GcHeap, _interner: &InternTable) -> NativeResult {
    Ok(args[0].clone()).into()
}

fn native_range(args: &[Value], heap: &mut GcHeap, _interner: &InternTable) -> NativeResult {
    (|| -> Result<Value, String> {
        match args.len() {
            1 => {
                // range(stop) → 0..stop
                let end = args[0].as_int().ok_or("range() requires int arguments")?;
                let r = heap.alloc(NiObject::Range(crate::object::NiRange {
                    start: 0,
                    end,
                    inclusive: false,
                    step: 1,
                }));
                Ok(Value::Object(r))
            }
            2 => {
                // range(start, stop) → start..stop
                let start = args[0].as_int().ok_or("range() requires int arguments")?;
                let end = args[1].as_int().ok_or("range() requires int arguments")?;
                let r = heap.alloc(NiObject::Range(crate::object::NiRange {
                    start,
                    end,
                    inclusive: false,
                    step: 1,
                }));
                Ok(Value::Object(r))
            }
            3 => {
                // range(start, stop, step)
                let start = args[0].as_int().ok_or("range() requires int arguments")?;
                let end = args[1].as_int().ok_or("range() requires int arguments")?;
                let step = args[2].as_int().ok_or("range() requires int arguments")?;
                if step == 0 {
                    return Err("range() step must not be zero".into());
                }
                let r = heap.alloc(NiObject::Range(crate::object::NiRange {
                    start,
                    end,
                    inclusive: false,
                    step,
                }));
                Ok(Value::Object(r))
            }
            _ => Err("range() takes 1 to 3 arguments".into()),
        }
    })()
    .into()
}

fn native_bytes(args: &[Value], heap: &mut GcHeap, _interner: &InternTable) -> NativeResult {
    (|| -> Result<Value, String> {
        if args.is_empty() {
            // Bytes() → empty bytes
            let r = heap.alloc(NiObject::Bytes(Vec::new()));
            return Ok(Value::Object(r));
        }
        match &args[0] {
            Value::Int(n) => {
                // Bytes(10) → zero-filled buffer of length 10
                if *n < 0 {
                    return Err("Bytes() size must be non-negative".into());
                }
                let size = *n as usize;
                if size > 100 * 1024 * 1024 {
                    return Err(format!("Bytes() size too large ({} bytes)", size).into());
                }
                let r = heap.alloc(NiObject::Bytes(vec![0u8; size]));
                Ok(Value::Object(r))
            }
            Value::Object(list_ref) => {
                // Bytes([1, 2, 3]) → bytes from list
                if let Some(NiObject::List(items)) = heap.get(*list_ref) {
                    let mut bytes = Vec::with_capacity(items.len());
                    for (i, val) in items.iter().enumerate() {
                        let n = val
                            .as_int()
                            .ok_or_else(|| format!("Bytes(): element {} is not an integer", i))?;
                        if !(0..=255).contains(&n) {
                            return Err(format!(
                                "Bytes(): element {} value {} out of range (0-255)",
                                i, n
                            ));
                        }
                        bytes.push(n as u8);
                    }
                    let r = heap.alloc(NiObject::Bytes(bytes));
                    Ok(Value::Object(r))
                } else {
                    Err("Bytes() requires an int size or a list of ints".into())
                }
            }
            _ => Err("Bytes() requires an int size or a list of ints".into()),
        }
    })()
    .into()
}

fn native_log(args: &[Value], heap: &mut GcHeap, interner: &InternTable) -> NativeResult {
    {
        let parts: Vec<String> = args
            .iter()
            .map(|v| value_to_display_string(v, heap, interner))
            .collect();
        println!("[log] {}", parts.join(" "));
        Ok(Value::None)
    }
    .into()
}

// --- Math functions for module ---

fn native_pow(args: &[Value], _heap: &mut GcHeap, _interner: &InternTable) -> NativeResult {
    (|| -> Result<Value, String> {
        let base = args[0].to_number().ok_or("pow() requires numbers")?;
        let exp = args[1].to_number().ok_or("pow() requires numbers")?;
        Ok(Value::Float(base.powf(exp)))
    })()
    .into()
}

fn native_atan2(args: &[Value], _heap: &mut GcHeap, _interner: &InternTable) -> NativeResult {
    (|| -> Result<Value, String> {
        let y = args[0].to_number().ok_or("atan2() requires numbers")?;
        let x = args[1].to_number().ok_or("atan2() requires numbers")?;
        Ok(Value::Float(y.atan2(x)))
    })()
    .into()
}

fn native_lerp(args: &[Value], _heap: &mut GcHeap, _interner: &InternTable) -> NativeResult {
    (|| -> Result<Value, String> {
        let a = args[0].to_number().ok_or("lerp() requires numbers")?;
        let b = args[1].to_number().ok_or("lerp() requires numbers")?;
        let t = args[2].to_number().ok_or("lerp() requires numbers")?;
        Ok(Value::Float(a + (b - a) * t))
    })()
    .into()
}

fn native_tan(args: &[Value], _heap: &mut GcHeap, _interner: &InternTable) -> NativeResult {
    (|| -> Result<Value, String> {
        let n = args[0].to_number().ok_or("tan() requires a number")?;
        Ok(Value::Float(n.tan()))
    })()
    .into()
}

fn native_asin(args: &[Value], _heap: &mut GcHeap, _interner: &InternTable) -> NativeResult {
    (|| -> Result<Value, String> {
        let n = args[0].to_number().ok_or("asin() requires a number")?;
        Ok(Value::Float(n.asin()))
    })()
    .into()
}

fn native_acos(args: &[Value], _heap: &mut GcHeap, _interner: &InternTable) -> NativeResult {
    (|| -> Result<Value, String> {
        let n = args[0].to_number().ok_or("acos() requires a number")?;
        Ok(Value::Float(n.acos()))
    })()
    .into()
}

fn native_atan(args: &[Value], _heap: &mut GcHeap, _interner: &InternTable) -> NativeResult {
    (|| -> Result<Value, String> {
        let n = args[0].to_number().ok_or("atan() requires a number")?;
        Ok(Value::Float(n.atan()))
    })()
    .into()
}

// --- Random functions for module ---

fn native_random_int(args: &[Value], _heap: &mut GcHeap, _interner: &InternTable) -> NativeResult {
    (|| -> Result<Value, String> {
        let min = args[0]
            .as_int()
            .ok_or("random.int() requires int arguments")?;
        let max = args[1]
            .as_int()
            .ok_or("random.int() requires int arguments")?;
        if min > max {
            return Err("random.int(): min must be <= max".into());
        }
        let range_i128 = max as i128 - min as i128 + 1;
        let val = if range_i128 > u64::MAX as i128 {
            // Full range - just use raw random value
            xorshift64() as i64
        } else {
            let range = range_i128 as u64;
            min + (xorshift64() % range) as i64
        };
        Ok(Value::Int(val))
    })()
    .into()
}

fn native_random_float(
    args: &[Value],
    _heap: &mut GcHeap,
    _interner: &InternTable,
) -> NativeResult {
    (|| -> Result<Value, String> {
        let min = args[0]
            .to_number()
            .ok_or("random.float() requires numbers")?;
        let max = args[1]
            .to_number()
            .ok_or("random.float() requires numbers")?;
        let val = min + random_f64() * (max - min);
        Ok(Value::Float(val))
    })()
    .into()
}

fn native_random_bool(
    _args: &[Value],
    _heap: &mut GcHeap,
    _interner: &InternTable,
) -> NativeResult {
    Ok(Value::Bool(xorshift64().is_multiple_of(2))).into()
}

fn native_random_chance(
    args: &[Value],
    _heap: &mut GcHeap,
    _interner: &InternTable,
) -> NativeResult {
    (|| -> Result<Value, String> {
        let p = args[0]
            .to_number()
            .ok_or("random.chance() requires a number")?;
        Ok(Value::Bool(random_f64() < p))
    })()
    .into()
}

fn native_random_choice(
    args: &[Value],
    heap: &mut GcHeap,
    _interner: &InternTable,
) -> NativeResult {
    (|| -> Result<Value, String> {
        if let Value::Object(r) = &args[0] {
            if let Some(NiObject::List(items)) = heap.get(*r) {
                if items.is_empty() {
                    return Err("random.choice() requires a non-empty list".into());
                }
                let idx = xorshift64() as usize % items.len();
                return Ok(items[idx].clone());
            }
        }
        Err("random.choice() requires a list".into())
    })()
    .into()
}

fn native_random_shuffle(
    args: &[Value],
    heap: &mut GcHeap,
    _interner: &InternTable,
) -> NativeResult {
    (|| -> Result<Value, String> {
        if let Value::Object(r) = &args[0] {
            if let Some(NiObject::List(items)) = heap.get_mut(*r) {
                // Fisher-Yates shuffle
                let len = items.len();
                for i in (1..len).rev() {
                    let j = xorshift64() as usize % (i + 1);
                    items.swap(i, j);
                }
                return Ok(Value::None);
            }
        }
        Err("random.shuffle() requires a list".into())
    })()
    .into()
}

fn native_random_seed(args: &[Value], _heap: &mut GcHeap, _interner: &InternTable) -> NativeResult {
    (|| -> Result<Value, String> {
        let seed = args[0].as_int().ok_or("random.seed() requires an int")?;
        let seed = if seed == 0 { 1u64 } else { seed as u64 };
        RNG_STATE.with(|state| {
            *state.borrow_mut() = seed;
        });
        Ok(Value::None)
    })()
    .into()
}

// --- Module builders ---

pub fn create_math_module(heap: &mut GcHeap) -> Vec<(Value, Value)> {
    let mut entries = Vec::new();

    let fns: Vec<(
        &str,
        i8,
        fn(&[Value], &mut GcHeap, &InternTable) -> NativeResult,
    )> = vec![
        ("abs", 1, native_abs),
        ("min", 2, native_min),
        ("max", 2, native_max),
        ("clamp", 3, native_clamp),
        ("sqrt", 1, native_sqrt),
        ("floor", 1, native_floor),
        ("ceil", 1, native_ceil),
        ("round", 1, native_round),
        ("sin", 1, native_sin),
        ("cos", 1, native_cos),
        ("tan", 1, native_tan),
        ("asin", 1, native_asin),
        ("acos", 1, native_acos),
        ("atan", 1, native_atan),
        ("atan2", 2, native_atan2),
        ("pow", 2, native_pow),
        ("lerp", 3, native_lerp),
    ];

    for (name, arity, func) in fns {
        let key = heap.alloc(NiObject::String(name.to_string()));
        let val = heap.alloc(NiObject::NativeFunction(NativeFn {
            name: name.to_string(),
            arity,
            function: func,
        }));
        entries.push((Value::Object(key), Value::Object(val)));
    }

    // Constants
    let pi_key = heap.alloc(NiObject::String("PI".to_string()));
    entries.push((Value::Object(pi_key), Value::Float(std::f64::consts::PI)));

    let tau_key = heap.alloc(NiObject::String("TAU".to_string()));
    entries.push((Value::Object(tau_key), Value::Float(std::f64::consts::TAU)));

    let inf_key = heap.alloc(NiObject::String("INF".to_string()));
    entries.push((Value::Object(inf_key), Value::Float(f64::INFINITY)));

    entries
}

pub fn create_random_module(heap: &mut GcHeap) -> Vec<(Value, Value)> {
    let mut entries = Vec::new();

    let fns: Vec<(
        &str,
        i8,
        fn(&[Value], &mut GcHeap, &InternTable) -> NativeResult,
    )> = vec![
        ("int", 2, native_random_int),
        ("float", 2, native_random_float),
        ("bool", 0, native_random_bool),
        ("chance", 1, native_random_chance),
        ("choice", 1, native_random_choice),
        ("shuffle", 1, native_random_shuffle),
        ("seed", 1, native_random_seed),
    ];

    for (name, arity, func) in fns {
        let key = heap.alloc(NiObject::String(name.to_string()));
        let val = heap.alloc(NiObject::NativeFunction(NativeFn {
            name: name.to_string(),
            arity,
            function: func,
        }));
        entries.push((Value::Object(key), Value::Object(val)));
    }

    entries
}

// --- Time functions for module ---

fn native_time_now(_args: &[Value], _heap: &mut GcHeap, _interner: &InternTable) -> NativeResult {
    (|| -> Result<Value, String> {
        use std::time::{SystemTime, UNIX_EPOCH};
        let secs = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|e| e.to_string())?
            .as_secs_f64();
        Ok(Value::Float(secs))
    })()
    .into()
}

fn native_time_since(args: &[Value], _heap: &mut GcHeap, _interner: &InternTable) -> NativeResult {
    (|| -> Result<Value, String> {
        use std::time::{SystemTime, UNIX_EPOCH};
        let start = args[0]
            .to_number()
            .ok_or("time.since() requires a number")?;
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|e| e.to_string())?
            .as_secs_f64();
        Ok(Value::Float(now - start))
    })()
    .into()
}

fn native_time_millis(
    _args: &[Value],
    _heap: &mut GcHeap,
    _interner: &InternTable,
) -> NativeResult {
    (|| -> Result<Value, String> {
        use std::time::{SystemTime, UNIX_EPOCH};
        let ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|e| e.to_string())?
            .as_millis();
        Ok(Value::Int(ms as i64))
    })()
    .into()
}

fn native_time_sleep(args: &[Value], _heap: &mut GcHeap, _interner: &InternTable) -> NativeResult {
    (|| -> Result<Value, String> {
        let secs = args[0]
            .to_number()
            .ok_or("time.sleep() requires a number")?;
        if !secs.is_finite() || secs < 0.0 {
            return Err("time.sleep() requires a finite non-negative number".into());
        }
        // Cap at 1 hour to prevent indefinite thread blocking
        if secs > 3600.0 {
            return Err(format!("time.sleep() duration too large ({:.1}s, max 3600s)", secs).into());
        }
        std::thread::sleep(std::time::Duration::from_secs_f64(secs));
        Ok(Value::None)
    })()
    .into()
}

pub fn create_time_module(heap: &mut GcHeap) -> Vec<(Value, Value)> {
    let mut entries = Vec::new();

    let fns: Vec<(
        &str,
        i8,
        fn(&[Value], &mut GcHeap, &InternTable) -> NativeResult,
    )> = vec![
        ("now", 0, native_time_now),
        ("since", 1, native_time_since),
        ("millis", 0, native_time_millis),
        ("sleep", 1, native_time_sleep),
    ];

    for (name, arity, func) in fns {
        let key = heap.alloc(NiObject::String(name.to_string()));
        let val = heap.alloc(NiObject::NativeFunction(NativeFn {
            name: name.to_string(),
            arity,
            function: func,
        }));
        entries.push((Value::Object(key), Value::Object(val)));
    }

    entries
}

// ============================================================
// JSON module -- hand-written recursive descent parser + encoder
// ============================================================

const MAX_JSON_DEPTH: usize = 256;

struct JsonParser {
    input: Vec<char>,
    pos: usize,
    depth: usize,
}

impl JsonParser {
    fn new(s: &str) -> Self {
        Self {
            input: s.chars().collect(),
            pos: 0,
            depth: 0,
        }
    }

    fn peek(&self) -> Option<char> {
        self.input.get(self.pos).copied()
    }

    fn advance(&mut self) -> Option<char> {
        let ch = self.input.get(self.pos).copied();
        self.pos += 1;
        ch
    }

    fn skip_ws(&mut self) {
        while let Some(ch) = self.peek() {
            if ch == ' ' || ch == '\t' || ch == '\n' || ch == '\r' {
                self.pos += 1;
            } else {
                break;
            }
        }
    }

    fn expect(&mut self, ch: char) -> Result<(), String> {
        self.skip_ws();
        match self.advance() {
            Some(c) if c == ch => Ok(()),
            Some(c) => Err(format!("Expected '{}', got '{}'", ch, c)),
            None => Err(format!("Expected '{}', got end of input", ch)),
        }
    }

    fn parse(&mut self, heap: &mut GcHeap) -> Result<Value, String> {
        let val = self.parse_value(heap)?;
        self.skip_ws();
        if self.pos < self.input.len() {
            return Err(format!(
                "Unexpected trailing content at position {}",
                self.pos
            ));
        }
        Ok(val)
    }

    fn parse_value(&mut self, heap: &mut GcHeap) -> Result<Value, String> {
        self.depth += 1;
        if self.depth > MAX_JSON_DEPTH {
            return Err("JSON nesting too deep (max 256)".to_string());
        }
        self.skip_ws();
        let result = match self.peek() {
            Some('{') => self.parse_object(heap),
            Some('[') => self.parse_array(heap),
            Some('"') => self.parse_string(heap).map(|s| {
                let r = heap.alloc(NiObject::String(s));
                Value::Object(r)
            }),
            Some('t') => {
                self.expect_literal("true")?;
                Ok(Value::Bool(true))
            }
            Some('f') => {
                self.expect_literal("false")?;
                Ok(Value::Bool(false))
            }
            Some('n') => {
                self.expect_literal("null")?;
                Ok(Value::None)
            }
            Some(c) if c == '-' || c.is_ascii_digit() => self.parse_number(),
            Some(c) => Err(format!(
                "Unexpected character '{}' at position {}",
                c, self.pos
            )),
            None => Err("Unexpected end of input".into()),
        };
        self.depth -= 1;
        result
    }

    fn expect_literal(&mut self, lit: &str) -> Result<(), String> {
        for expected in lit.chars() {
            match self.advance() {
                Some(c) if c == expected => {}
                _ => return Err(format!("Expected '{}'", lit)),
            }
        }
        Ok(())
    }

    fn parse_object(&mut self, heap: &mut GcHeap) -> Result<Value, String> {
        self.advance(); // consume '{'
        self.skip_ws();
        let mut entries = Vec::new();
        if self.peek() == Some('}') {
            self.advance();
            let r = heap.alloc(NiObject::Map(entries));
            return Ok(Value::Object(r));
        }
        loop {
            self.skip_ws();
            if self.peek() != Some('"') {
                return Err("Expected string key in object".into());
            }
            let key_str = self.parse_string(heap)?;
            let key = heap.alloc(NiObject::String(key_str));
            self.expect(':')?;
            let val = self.parse_value(heap)?;
            entries.push((Value::Object(key), val));
            self.skip_ws();
            match self.peek() {
                Some(',') => {
                    self.advance();
                }
                Some('}') => {
                    self.advance();
                    break;
                }
                _ => return Err("Expected ',' or '}' in object".into()),
            }
        }
        let r = heap.alloc(NiObject::Map(entries));
        Ok(Value::Object(r))
    }

    fn parse_array(&mut self, heap: &mut GcHeap) -> Result<Value, String> {
        self.advance(); // consume '['
        self.skip_ws();
        let mut items = Vec::new();
        if self.peek() == Some(']') {
            self.advance();
            let r = heap.alloc(NiObject::List(items));
            return Ok(Value::Object(r));
        }
        loop {
            let val = self.parse_value(heap)?;
            items.push(val);
            self.skip_ws();
            match self.peek() {
                Some(',') => {
                    self.advance();
                }
                Some(']') => {
                    self.advance();
                    break;
                }
                _ => return Err("Expected ',' or ']' in array".into()),
            }
        }
        let r = heap.alloc(NiObject::List(items));
        Ok(Value::Object(r))
    }

    fn parse_string(&mut self, _heap: &mut GcHeap) -> Result<String, String> {
        self.expect('"')?;
        let mut s = String::new();
        loop {
            match self.advance() {
                Some('"') => return Ok(s),
                Some('\\') => match self.advance() {
                    Some('"') => s.push('"'),
                    Some('\\') => s.push('\\'),
                    Some('/') => s.push('/'),
                    Some('n') => s.push('\n'),
                    Some('t') => s.push('\t'),
                    Some('r') => s.push('\r'),
                    Some('b') => s.push('\u{08}'),
                    Some('f') => s.push('\u{0C}'),
                    Some('u') => {
                        let mut hex = String::new();
                        for _ in 0..4 {
                            match self.advance() {
                                Some(c) if c.is_ascii_hexdigit() => hex.push(c),
                                _ => return Err("Invalid \\uXXXX escape".into()),
                            }
                        }
                        let cp =
                            u32::from_str_radix(&hex, 16).map_err(|_| "Invalid \\uXXXX escape")?;
                        if (0xD800..=0xDBFF).contains(&cp) {
                            // High surrogate — expect \uXXXX low surrogate
                            if self.advance() == Some('\\') && self.advance() == Some('u') {
                                let mut low_hex = String::new();
                                for _ in 0..4 {
                                    match self.advance() {
                                        Some(c) if c.is_ascii_hexdigit() => low_hex.push(c),
                                        _ => return Err("Invalid \\uXXXX escape in surrogate pair".into()),
                                    }
                                }
                                let low = u32::from_str_radix(&low_hex, 16)
                                    .map_err(|_| format!("Invalid unicode escape: \\u{}", low_hex))?;
                                if !(0xDC00..=0xDFFF).contains(&low) {
                                    return Err(format!("Invalid surrogate pair: \\u{} \\u{}", hex, low_hex));
                                }
                                let combined = 0x10000 + ((cp - 0xD800) << 10) + (low - 0xDC00);
                                let ch = char::from_u32(combined)
                                    .ok_or_else(|| format!("Invalid unicode codepoint: {:X}", combined))?;
                                s.push(ch);
                            } else {
                                return Err(format!("Expected low surrogate after \\u{}", hex));
                            }
                        } else if (0xDC00..=0xDFFF).contains(&cp) {
                            return Err(format!("Unexpected low surrogate: \\u{}", hex));
                        } else {
                            let ch = char::from_u32(cp)
                                .ok_or_else(|| format!("Invalid unicode codepoint: {}", hex))?;
                            s.push(ch);
                        }
                    }
                    Some(c) => return Err(format!("Invalid escape: \\{}", c)),
                    None => return Err("Unterminated string".into()),
                },
                Some(c) => s.push(c),
                None => return Err("Unterminated string".into()),
            }
        }
    }

    fn parse_number(&mut self) -> Result<Value, String> {
        let start = self.pos;
        if self.peek() == Some('-') {
            self.advance();
        }
        if self.peek() == Some('0') {
            self.advance();
        } else {
            if !self.peek().is_some_and(|c| c.is_ascii_digit()) {
                return Err("Invalid number".into());
            }
            while self.peek().is_some_and(|c| c.is_ascii_digit()) {
                self.advance();
            }
        }
        let mut is_float = false;
        if self.peek() == Some('.') {
            is_float = true;
            self.advance();
            if !self.peek().is_some_and(|c| c.is_ascii_digit()) {
                return Err("Invalid number: expected digit after '.'".into());
            }
            while self.peek().is_some_and(|c| c.is_ascii_digit()) {
                self.advance();
            }
        }
        if self.peek() == Some('e') || self.peek() == Some('E') {
            is_float = true;
            self.advance();
            if self.peek() == Some('+') || self.peek() == Some('-') {
                self.advance();
            }
            if !self.peek().is_some_and(|c| c.is_ascii_digit()) {
                return Err("Invalid number: expected digit in exponent".into());
            }
            while self.peek().is_some_and(|c| c.is_ascii_digit()) {
                self.advance();
            }
        }
        let num_str: String = self.input[start..self.pos].iter().collect();
        if is_float {
            let f: f64 = num_str
                .parse()
                .map_err(|_| format!("Invalid number: {}", num_str))?;
            Ok(Value::Float(f))
        } else {
            // Try int first, fall back to float for very large numbers
            if let Ok(n) = num_str.parse::<i64>() {
                Ok(Value::Int(n))
            } else {
                let f: f64 = num_str
                    .parse()
                    .map_err(|_| format!("Invalid number: {}", num_str))?;
                Ok(Value::Float(f))
            }
        }
    }
}

const MAX_ENCODE_DEPTH: usize = 256;

fn json_encode(val: &Value, heap: &GcHeap, interner: &InternTable, depth: usize) -> Result<String, String> {
    if depth > MAX_ENCODE_DEPTH {
        return Err("JSON encode exceeded maximum nesting depth (circular reference or too deeply nested)".into());
    }
    match val {
        Value::Int(n) => Ok(format!("{}", n)),
        Value::Float(f) => {
            if f.is_nan() || f.is_infinite() {
                return Err("Cannot encode NaN or Infinity to JSON".into());
            }
            if *f == f.floor() && f.is_finite() && f.abs() < 1e15 {
                Ok(format!("{:.1}", f))
            } else {
                Ok(format!("{}", f))
            }
        }
        Value::Bool(b) => Ok(if *b { "true".into() } else { "false".into() }),
        Value::None => Ok("null".into()),
        Value::Object(r) => {
            if let Some(obj) = heap.get(*r) {
                match obj {
                    NiObject::String(s) => Ok(json_escape_string(s)),
                    NiObject::InternedString(id) => Ok(json_escape_string(interner.resolve(*id))),
                    NiObject::List(items) => {
                        let items = items.clone();
                        let parts: Result<Vec<String>, String> = items
                            .iter()
                            .map(|v| json_encode(v, heap, interner, depth + 1))
                            .collect();
                        Ok(format!("[{}]", parts?.join(", ")))
                    }
                    NiObject::Map(entries) => {
                        let entries = entries.clone();
                        let mut parts = Vec::new();
                        for (k, v) in &entries {
                            let key_str = match k {
                                Value::Object(kr) => {
                                    if let Some(ko) = heap.get(*kr) {
                                        ko.as_string_with_intern(interner)
                                            .map(|s| s.to_string())
                                            .ok_or_else(|| {
                                                "JSON object keys must be strings".to_string()
                                            })?
                                    } else {
                                        return Err("Invalid reference in map key".into());
                                    }
                                }
                                _ => return Err("JSON object keys must be strings".into()),
                            };
                            parts.push(format!(
                                "{}: {}",
                                json_escape_string(&key_str),
                                json_encode(v, heap, interner, depth + 1)?
                            ));
                        }
                        Ok(format!("{{{}}}", parts.join(", ")))
                    }
                    _ => Err(format!("Cannot encode {} to JSON", obj.type_name())),
                }
            } else {
                Err("Invalid reference".into())
            }
        }
    }
}

fn json_escape_string(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for ch in s.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\t' => out.push_str("\\t"),
            '\r' => out.push_str("\\r"),
            '\u{08}' => out.push_str("\\b"),
            '\u{0C}' => out.push_str("\\f"),
            c if (c as u32) < 0x20 => {
                out.push_str(&format!("\\u{:04x}", c as u32));
            }
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

fn native_json_parse(args: &[Value], heap: &mut GcHeap, interner: &InternTable) -> NativeResult {
    (|| -> Result<Value, String> {
        let s = match &args[0] {
            Value::Object(r) => {
                if let Some(obj) = heap.get(*r) {
                    obj.as_string_with_intern(interner)
                        .map(|s| s.to_string())
                        .ok_or_else(|| "json.parse() requires a string argument".to_string())
                } else {
                    Err("Invalid reference".into())
                }
            }
            _ => Err("json.parse() requires a string argument".into()),
        }?;
        let mut parser = JsonParser::new(&s);
        parser.parse(heap)
    })()
    .into()
}

fn native_json_encode(args: &[Value], heap: &mut GcHeap, interner: &InternTable) -> NativeResult {
    (|| -> Result<Value, String> {
        let result = json_encode(&args[0], heap, interner, 0)?;
        let r = heap.alloc(NiObject::String(result));
        Ok(Value::Object(r))
    })()
    .into()
}

pub fn create_json_module(heap: &mut GcHeap) -> Vec<(Value, Value)> {
    let mut entries = Vec::new();

    let fns: Vec<(
        &str,
        i8,
        fn(&[Value], &mut GcHeap, &InternTable) -> NativeResult,
    )> = vec![
        ("parse", 1, native_json_parse),
        ("encode", 1, native_json_encode),
    ];

    for (name, arity, func) in fns {
        let key = heap.alloc(NiObject::String(name.to_string()));
        let val = heap.alloc(NiObject::NativeFunction(NativeFn {
            name: name.to_string(),
            arity,
            function: func,
        }));
        entries.push((Value::Object(key), Value::Object(val)));
    }

    entries
}

// ============================================================
// NiON module -- hand-written parser + encoder for Ni object notation
// ============================================================

struct NionParser {
    input: Vec<char>,
    pos: usize,
    depth: usize,
}

impl NionParser {
    fn new(s: &str) -> Self {
        Self {
            input: s.chars().collect(),
            pos: 0,
            depth: 0,
        }
    }

    fn peek(&self) -> Option<char> {
        self.input.get(self.pos).copied()
    }

    fn advance(&mut self) -> Option<char> {
        let ch = self.input.get(self.pos).copied();
        self.pos += 1;
        ch
    }

    fn skip_ws(&mut self) {
        while let Some(ch) = self.peek() {
            if ch == ' ' || ch == '\t' || ch == '\n' || ch == '\r' {
                self.pos += 1;
            } else {
                break;
            }
        }
    }

    fn expect(&mut self, ch: char) -> Result<(), String> {
        self.skip_ws();
        match self.advance() {
            Some(c) if c == ch => Ok(()),
            Some(c) => Err(format!("Expected '{}', got '{}'", ch, c)),
            None => Err(format!("Expected '{}', got end of input", ch)),
        }
    }

    fn parse(&mut self, heap: &mut GcHeap) -> Result<Value, String> {
        let val = self.parse_value(heap)?;
        self.skip_ws();
        if self.pos < self.input.len() {
            return Err(format!(
                "Unexpected trailing content at position {}",
                self.pos
            ));
        }
        Ok(val)
    }

    fn parse_value(&mut self, heap: &mut GcHeap) -> Result<Value, String> {
        self.depth += 1;
        if self.depth > MAX_JSON_DEPTH {
            return Err("NiON nesting too deep (max 256)".to_string());
        }
        self.skip_ws();
        let result = match self.peek() {
            Some('[') => self.parse_bracket(heap),
            Some('{') => self.parse_brace_map(heap),
            Some('"') => {
                let s = self.parse_string()?;
                let r = heap.alloc(NiObject::String(s));
                Ok(Value::Object(r))
            }
            Some('t') => {
                self.expect_literal("true")?;
                Ok(Value::Bool(true))
            }
            Some('f') => {
                self.expect_literal("false")?;
                Ok(Value::Bool(false))
            }
            Some('n') => {
                // Accept both "none" and "null"
                if self.input.get(self.pos + 1) == Some(&'o') {
                    self.expect_literal("none")?;
                } else {
                    self.expect_literal("null")?;
                }
                Ok(Value::None)
            }
            Some(c) if c == '-' || c.is_ascii_digit() => self.parse_number(),
            Some(c) => Err(format!(
                "Unexpected character '{}' at position {}",
                c, self.pos
            )),
            None => Err("Unexpected end of input".into()),
        };
        self.depth -= 1;
        result
    }

    fn expect_literal(&mut self, lit: &str) -> Result<(), String> {
        for expected in lit.chars() {
            match self.advance() {
                Some(c) if c == expected => {}
                _ => return Err(format!("Expected '{}'", lit)),
            }
        }
        Ok(())
    }

    fn parse_bracket(&mut self, heap: &mut GcHeap) -> Result<Value, String> {
        self.advance(); // consume '['
        self.skip_ws();

        // Empty map [:] or empty list []
        if self.peek() == Some(':') {
            self.advance();
            self.expect(']')?;
            let r = heap.alloc(NiObject::Map(Vec::new()));
            return Ok(Value::Object(r));
        }
        if self.peek() == Some(']') {
            self.advance();
            let r = heap.alloc(NiObject::List(Vec::new()));
            return Ok(Value::Object(r));
        }

        // Parse first value, then check for ':' to decide map vs list
        let first = self.parse_value(heap)?;
        self.skip_ws();

        if self.peek() == Some(':') {
            // Map
            self.advance();
            let first_val = self.parse_value(heap)?;
            let mut entries = vec![(first, first_val)];
            self.skip_ws();
            while self.peek() == Some(',') {
                self.advance();
                self.skip_ws();
                if self.peek() == Some(']') {
                    break;
                }
                let key = self.parse_value(heap)?;
                self.expect(':')?;
                let val = self.parse_value(heap)?;
                entries.push((key, val));
                self.skip_ws();
            }
            self.expect(']')?;
            let r = heap.alloc(NiObject::Map(entries));
            Ok(Value::Object(r))
        } else {
            // List
            let mut items = vec![first];
            while self.peek() == Some(',') {
                self.advance();
                self.skip_ws();
                if self.peek() == Some(']') {
                    break;
                }
                items.push(self.parse_value(heap)?);
                self.skip_ws();
            }
            self.expect(']')?;
            let r = heap.alloc(NiObject::List(items));
            Ok(Value::Object(r))
        }
    }

    fn parse_brace_map(&mut self, heap: &mut GcHeap) -> Result<Value, String> {
        self.advance(); // consume '{'
        self.skip_ws();
        let mut entries = Vec::new();
        if self.peek() == Some('}') {
            self.advance();
            let r = heap.alloc(NiObject::Map(entries));
            return Ok(Value::Object(r));
        }
        loop {
            let key = self.parse_value(heap)?;
            self.expect(':')?;
            let val = self.parse_value(heap)?;
            entries.push((key, val));
            self.skip_ws();
            match self.peek() {
                Some(',') => {
                    self.advance();
                    self.skip_ws();
                    if self.peek() == Some('}') {
                        self.advance();
                        break;
                    }
                }
                Some('}') => {
                    self.advance();
                    break;
                }
                _ => return Err("Expected ',' or '}' in map".into()),
            }
        }
        let r = heap.alloc(NiObject::Map(entries));
        Ok(Value::Object(r))
    }

    fn parse_string(&mut self) -> Result<String, String> {
        self.expect('"')?;
        let mut s = String::new();
        loop {
            match self.advance() {
                Some('"') => return Ok(s),
                Some('\\') => match self.advance() {
                    Some('"') => s.push('"'),
                    Some('\\') => s.push('\\'),
                    Some('n') => s.push('\n'),
                    Some('t') => s.push('\t'),
                    Some('r') => s.push('\r'),
                    Some(c) => {
                        s.push('\\');
                        s.push(c);
                    }
                    None => return Err("Unterminated string".into()),
                },
                Some(c) => s.push(c),
                None => return Err("Unterminated string".into()),
            }
        }
    }

    fn parse_number(&mut self) -> Result<Value, String> {
        let start = self.pos;
        if self.peek() == Some('-') {
            self.advance();
        }
        if !self.peek().is_some_and(|c| c.is_ascii_digit()) {
            return Err("Invalid number: expected digit".into());
        }
        while self.peek().is_some_and(|c| c.is_ascii_digit()) {
            self.advance();
        }
        let mut is_float = false;
        if self.peek() == Some('.') {
            is_float = true;
            self.advance();
            while self.peek().is_some_and(|c| c.is_ascii_digit()) {
                self.advance();
            }
        }
        if self.peek() == Some('e') || self.peek() == Some('E') {
            is_float = true;
            self.advance();
            if self.peek() == Some('+') || self.peek() == Some('-') {
                self.advance();
            }
            while self.peek().is_some_and(|c| c.is_ascii_digit()) {
                self.advance();
            }
        }
        let num_str: String = self.input[start..self.pos].iter().collect();
        if is_float {
            let f: f64 = num_str
                .parse()
                .map_err(|_| format!("Invalid number: {}", num_str))?;
            Ok(Value::Float(f))
        } else if let Ok(n) = num_str.parse::<i64>() {
            Ok(Value::Int(n))
        } else {
            let f: f64 = num_str
                .parse()
                .map_err(|_| format!("Invalid number: {}", num_str))?;
            Ok(Value::Float(f))
        }
    }
}

fn nion_encode(val: &Value, heap: &GcHeap, interner: &InternTable, depth: usize) -> Result<String, String> {
    if depth > MAX_ENCODE_DEPTH {
        return Err("NiON encode exceeded maximum nesting depth (circular reference or too deeply nested)".into());
    }
    match val {
        Value::Int(n) => Ok(format!("{}", n)),
        Value::Float(f) => {
            if f.is_nan() || f.is_infinite() {
                return Err("Cannot encode NaN or Infinity to NiON".into());
            }
            if *f == f.floor() && f.is_finite() && f.abs() < 1e15 {
                Ok(format!("{:.1}", f))
            } else {
                Ok(format!("{}", f))
            }
        }
        Value::Bool(b) => Ok(if *b { "true".into() } else { "false".into() }),
        Value::None => Ok("none".into()),
        Value::Object(r) => {
            if let Some(obj) = heap.get(*r) {
                match obj {
                    NiObject::String(s) => Ok(nion_escape_string(s)),
                    NiObject::InternedString(id) => Ok(nion_escape_string(interner.resolve(*id))),
                    NiObject::List(items) => {
                        let items = items.clone();
                        let parts: Result<Vec<String>, String> = items
                            .iter()
                            .map(|v| nion_encode(v, heap, interner, depth + 1))
                            .collect();
                        Ok(format!("[{}]", parts?.join(", ")))
                    }
                    NiObject::Map(entries) => {
                        let entries = entries.clone();
                        if entries.is_empty() {
                            return Ok("[:]".to_string());
                        }
                        let mut parts = Vec::new();
                        for (k, v) in &entries {
                            parts.push(format!(
                                "{}: {}",
                                nion_encode(k, heap, interner, depth + 1)?,
                                nion_encode(v, heap, interner, depth + 1)?
                            ));
                        }
                        Ok(format!("[{}]", parts.join(", ")))
                    }
                    _ => Err(format!("Cannot encode {} to NiON", obj.type_name())),
                }
            } else {
                Err("Invalid reference".into())
            }
        }
    }
}

fn nion_escape_string(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for ch in s.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\t' => out.push_str("\\t"),
            '\r' => out.push_str("\\r"),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

fn native_nion_parse(args: &[Value], heap: &mut GcHeap, interner: &InternTable) -> NativeResult {
    (|| -> Result<Value, String> {
        let s = match &args[0] {
            Value::Object(r) => {
                if let Some(obj) = heap.get(*r) {
                    obj.as_string_with_intern(interner)
                        .map(|s| s.to_string())
                        .ok_or_else(|| "nion.parse() requires a string argument".to_string())
                } else {
                    Err("Invalid reference".into())
                }
            }
            _ => Err("nion.parse() requires a string argument".into()),
        }?;
        let mut parser = NionParser::new(&s);
        parser.parse(heap)
    })()
    .into()
}

fn native_nion_encode(args: &[Value], heap: &mut GcHeap, interner: &InternTable) -> NativeResult {
    (|| -> Result<Value, String> {
        let result = nion_encode(&args[0], heap, interner, 0)?;
        let r = heap.alloc(NiObject::String(result));
        Ok(Value::Object(r))
    })()
    .into()
}

pub fn create_nion_module(heap: &mut GcHeap) -> Vec<(Value, Value)> {
    let mut entries = Vec::new();

    let fns: Vec<(
        &str,
        i8,
        fn(&[Value], &mut GcHeap, &InternTable) -> NativeResult,
    )> = vec![
        ("parse", 1, native_nion_parse),
        ("encode", 1, native_nion_encode),
    ];

    for (name, arity, func) in fns {
        let key = heap.alloc(NiObject::String(name.to_string()));
        let val = heap.alloc(NiObject::NativeFunction(NativeFn {
            name: name.to_string(),
            arity,
            function: func,
        }));
        entries.push((Value::Object(key), Value::Object(val)));
    }

    entries
}

pub fn value_to_display_string(val: &Value, heap: &GcHeap, interner: &InternTable) -> String {
    match val {
        Value::Object(r) => {
            if let Some(obj) = heap.get(*r) {
                match obj {
                    NiObject::String(s) => s.clone(),
                    NiObject::InternedString(id) => interner.resolve(*id).to_string(),
                    _ => obj.display(heap, interner),
                }
            } else {
                "<freed>".to_string()
            }
        }
        _ => format!("{}", val),
    }
}
