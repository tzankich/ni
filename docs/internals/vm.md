# Bytecode VM Specification

## Overview

The Ni VM is a stack-based bytecode interpreter written in Rust. It runs inside the host engine's main loop.

## Value Representation

All values on the stack are tagged unions (NaN-boxed for performance):

```
64-bit value:
  - Float:    IEEE 754 double (NaN bits clear)
  - Int:      51-bit signed integer (NaN-boxed)
  - Bool:     singleton true/false
  - None:     singleton
  - Object:   pointer to heap object (String, InternedString, List, Map, Instance, Fiber, Function)
```

NaN-boxing allows the VM to store all values in a uniform 8-byte slot, with type checks being simple bit masks. This is the same technique used by LuaJIT, JavaScriptCore, and Wren.

### Rust Implementation Notes

NaN-boxing in Rust requires `unsafe` for the bit manipulation, but this is localized to the `NiValue` type's implementation. The public API is safe:

```rust
#[derive(Clone, Copy)]
pub struct NiValue(u64);  // NaN-boxed

impl NiValue {
    pub fn int(v: i64) -> Self { /* NaN-box encoding */ }
    pub fn float(v: f64) -> Self { /* raw bits */ }
    pub fn as_int(&self) -> Option<i64> { /* NaN-box decoding */ }
    pub fn is_object(&self) -> bool { /* tag check */ }
    // ...
}
```

**Alternative approach (recommended for v0.1):** Start with a tagged enum and benchmark. Optimize to NaN-boxing only if profiling shows value dispatch is a bottleneck:

```rust
#[derive(Clone, Debug)]
pub enum NiValue {
    Int(i64),
    Float(f64),
    Bool(bool),
    None,
    Object(GcRef<NiObject>),  // GC-managed heap reference
}
```

The enum approach is idiomatic Rust, fully safe, and pattern-matching-friendly. It's ~2x larger per value (16 bytes vs 8) but more debuggable. The migration path to NaN-boxing is mechanical: swap the `NiValue` type, keep the same public API.

**GC strategy for Rust:** Use an arena allocator with generational indices instead of raw pointers. This avoids fighting the borrow checker:

```rust
pub struct GcHeap {
    objects: Vec<Option<NiObject>>,  // arena
    free_list: Vec<usize>,
}

#[derive(Clone, Copy)]
pub struct GcRef<T> {
    index: u32,
    generation: u32,  // detects use-after-free
    _phantom: PhantomData<T>,
}
```

This pattern (used by `slotmap`, `generational-arena`, and many Rust ECS frameworks) gives O(1) allocation, safe references, and straightforward mark-and-sweep. The GC walks the arena, marks reachable objects, and frees unmarked slots.

## Opcodes

Target: ~40 opcodes for v0.1. Grouped by category:

### Stack Operations

| Opcode | Args | Description |
|--------|------|-------------|
| `OP_CONST` | index(u16) | Push constant from pool |
| `OP_NONE` | -- | Push `none` |
| `OP_TRUE` | -- | Push `true` |
| `OP_FALSE` | -- | Push `false` |
| `OP_POP` | -- | Pop and discard top |
| `OP_DUP` | -- | Duplicate top of stack |

### Local Variables

| Opcode | Args | Description |
|--------|------|-------------|
| `OP_GET_LOCAL` | slot(u8) | Push local variable |
| `OP_SET_LOCAL` | slot(u8) | Set local variable |
| `OP_GET_GLOBAL` | name(u16) | Push global variable |
| `OP_SET_GLOBAL` | name(u16) | Set global variable |

### Arithmetic

| Opcode | Args | Description |
|--------|------|-------------|
| `OP_ADD` | -- | a + b |
| `OP_SUB` | -- | a - b |
| `OP_MUL` | -- | a * b |
| `OP_DIV` | -- | a / b |
| `OP_MOD` | -- | a % b |
| `OP_NEG` | -- | -a |

### Comparison

| Opcode | Args | Description |
|--------|------|-------------|
| `OP_EQ` | -- | a == b |
| `OP_NEQ` | -- | a != b |
| `OP_LT` | -- | a < b |
| `OP_GT` | -- | a > b |
| `OP_LTE` | -- | a <= b |
| `OP_GTE` | -- | a >= b |

### Logic

| Opcode | Args | Description |
|--------|------|-------------|
| `OP_NOT` | -- | logical not |
| `OP_AND` | offset(u16) | short-circuit AND |
| `OP_OR` | offset(u16) | short-circuit OR |

### Control Flow

| Opcode | Args | Description |
|--------|------|-------------|
| `OP_JUMP` | offset(i16) | unconditional jump |
| `OP_JUMP_IF_FALSE` | offset(i16) | conditional jump |
| `OP_LOOP` | offset(u16) | jump backward (loops) |

### Functions & Methods

| Opcode | Args | Description |
|--------|------|-------------|
| `OP_CALL` | arg_count(u8) | call function |
| `OP_RETURN` | -- | return from function |
| `OP_CLOSURE` | fn(u16), upvalues... | create closure |
| `OP_GET_UPVALUE` | slot(u8) | read closed-over variable |
| `OP_SET_UPVALUE` | slot(u8) | write closed-over variable |
| `OP_CLOSE_UPVALUE` | -- | close over local going out of scope |

### Objects & Properties

| Opcode | Args | Description |
|--------|------|-------------|
| `OP_GET_PROP` | name(u16) | obj.property |
| `OP_SET_PROP` | name(u16) | obj.property = val |
| `OP_GET_INDEX` | -- | obj[index] |
| `OP_SET_INDEX` | -- | obj[index] = val |
| `OP_CLASS` | name(u16) | define class |
| `OP_METHOD` | name(u16) | add method to class |
| `OP_INHERIT` | -- | set up inheritance |
| `OP_INVOKE` | name(u16), args(u8) | optimized method call |

### Collections

| Opcode | Args | Description |
|--------|------|-------------|
| `OP_LIST` | count(u16) | create list from N stack values |
| `OP_MAP` | count(u16) | create map from N key-value pairs |

### Coroutines / Fibers

| Opcode | Args | Description |
|--------|------|-------------|
| `SpawnFiber` | -- | pop closure, create fiber in ready queue, push FiberId |
| `Yield` | -- | pop value, store in fiber.result, suspend fiber |
| `Wait` | -- | pop duration (seconds), set wait_timer, suspend fiber |
| `Await` | -- | if last native returned Pending, park fiber; otherwise no-op |

## Bytecode Format

```
NI! bytecode file (.nic):

Header (16 bytes):
  Magic:     "NI!\0"  (4 bytes)
  Version:   u16      (bytecode version)
  Flags:     u16      (debug info present, etc.)
  Checksum:  u32      (CRC32 of everything after header)
  Reserved:  u32

Constant Pool:
  count:     u16
  entries:   [tag(u8) + data...]
    INT:     0x01 + i64 (8 bytes)
    FLOAT:   0x02 + f64 (8 bytes)
    STRING:  0x03 + length(u16) + utf8_bytes
    FUNC:    0x04 + function_descriptor

Function Descriptors:
  name:          string_index (u16)
  arity:         u8
  upvalue_count: u8
  code_length:   u32
  code:          [u8...]
  line_info:     [u16...] (line number for each instruction, debug only)

Module:
  name:          string_index
  global_count:  u16
  functions:     [function_descriptors...]
  entry_point:   function_index (u16)  # top-level code
```

## Execution Model

Each fiber has its own:
- **Call stack:** Stack of call frames (function + instruction pointer + stack base).
- **Value stack:** Operand stack for the current computation.
- **State:** Running, Suspended, Finished, Cancelled.

The engine's main loop:
1. Process input.
2. Run physics/collision.
3. **Resume all active fibers** (each gets a time slice or runs until yield/wait).
4. Render.

Fiber scheduling is cooperative: a fiber runs until it hits `wait`, `yield`, or finishes. There is no preemption within a single frame. The engine enforces a per-frame instruction limit (default: 100,000 instructions) to catch infinite loops.

## String Interning

The VM uses **string interning** for all identifier-keyed lookups. An `InternTable` maps strings to compact `InternId` values (u32 newtypes), enabling O(1) integer hash+compare for property access, method dispatch, global variables, and enum variants -- instead of variable-length string hashing and byte-by-byte comparison.

### Intern Table

The `InternTable` lives outside the GC heap (interned strings are permanent, never collected). It provides bidirectional mapping:

- `intern(s) -> InternId` -- returns existing ID or creates a new one
- `resolve(id) -> &str` -- zero-allocation lookup from ID to string

### Interned vs Dynamic Strings

String values in the VM have two representations:

| Variant | Source | Use Case |
|---------|--------|----------|
| `InternedString(InternId)` | String literals, identifiers in source code | Constant strings from compilation |
| `String(String)` | Concatenation, interpolation, user input | Dynamically constructed strings |

Both variants are transparent to user code -- they behave identically for all string operations (methods, equality, indexing, iteration). The VM resolves the appropriate representation internally.

### InternId-Keyed Data Structures

All internal lookup tables use `InternId` keys instead of `String` keys:

- `globals: HashMap<InternId, Value>` -- global variables
- `NiClass.methods: HashMap<InternId, GcRef>` -- class methods
- `NiClass.fields: HashMap<InternId, Value>` -- class default field values
- `NiInstance.fields: HashMap<InternId, Value>` -- instance fields
- `NiEnum.variants: HashMap<InternId, Value>` -- enum variants

The constructor name `"init"` is pre-interned at VM startup to avoid repeated lookups during class instantiation.

### Equality Semantics

String equality compares by content regardless of representation. Two strings are equal if they contain the same bytes, whether both are interned, both are dynamic, or one of each. When both operands are `InternedString`, equality reduces to a single u32 comparison.

## Garbage Collection

The VM uses **tracing garbage collection** (mark-and-sweep) with an arena-based allocator:

- All heap objects (dynamic strings, lists, maps, instances, fibers, closures) are GC-managed. Interned strings live in the `InternTable` outside the GC heap and are never collected (see String Interning above).
- Objects live in a `GcHeap` arena (see Rust Implementation Notes above). Generational indices provide safe references without raw pointers.
- GC roots: global variables, all fiber stacks, open upvalues.
- GC runs incrementally (a little each frame) to avoid frame-time spikes. The sweep budget is configurable (default: 1ms per frame).
- Memory ceiling: scripts cannot allocate more than a configurable limit (default: 32MB). Exceeding the limit triggers a `MemoryExceeded` error that terminates the offending fiber.
- **Crate recommendation:** Consider `gc-arena` (used by Ruffle, the Flash emulator in Rust) which provides a safe GC specifically designed for language VMs in Rust. Alternatively, `slotmap` or `generational-arena` for the manual approach.
