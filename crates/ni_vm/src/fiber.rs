use std::sync::atomic::{AtomicU64, Ordering};

use crate::gc::GcRef;
use crate::value::Value;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct FiberId(pub u64);

static FIBER_ID_COUNTER: AtomicU64 = AtomicU64::new(1);

impl FiberId {
    pub fn next() -> Self {
        FiberId(FIBER_ID_COUNTER.fetch_add(1, Ordering::Relaxed))
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum FiberState {
    Created,
    Running,
    Suspended,
    Parked,
    Finished,
    Cancelled,
}

#[derive(Debug, Clone)]
pub struct CallFrame {
    pub closure: GcRef,
    pub ip: usize,
    pub stack_base: usize,
}

#[derive(Debug, Clone)]
pub struct CatchPoint {
    pub stack_size: usize, // runtime stack size when SetCatchPoint executed
    pub frame_idx: usize,  // call frame index
    pub handler_ip: usize, // absolute IP to jump to on fail
}

#[derive(Debug, Clone)]
pub struct Fiber {
    pub stack: Vec<Value>,
    pub frames: Vec<CallFrame>,
    pub state: FiberState,
    pub open_upvalues: Vec<GcRef>,
    pub catch_points: Vec<CatchPoint>,
    pub wait_timer: f64,
    pub result: Option<Value>,
}

impl Fiber {
    pub fn new(closure: GcRef) -> Self {
        Self {
            stack: vec![Value::Object(closure)], // slot 0 is the function itself
            frames: vec![CallFrame {
                closure,
                ip: 0,
                stack_base: 0,
            }],
            state: FiberState::Created,
            open_upvalues: Vec::new(),
            catch_points: Vec::new(),
            wait_timer: 0.0,
            result: None,
        }
    }

    /// Create an empty finished fiber (placeholder).
    pub fn empty() -> Self {
        Self {
            stack: Vec::new(),
            frames: Vec::new(),
            state: FiberState::Finished,
            open_upvalues: Vec::new(),
            catch_points: Vec::new(),
            wait_timer: 0.0,
            result: None,
        }
    }

    pub fn push(&mut self, value: Value) {
        self.stack.push(value);
    }

    pub fn pop(&mut self) -> Value {
        self.stack.pop().unwrap_or(Value::None)
    }

    pub fn peek(&self, distance: usize) -> &Value {
        const NONE: Value = Value::None;
        match self.stack.len().checked_sub(1 + distance) {
            Some(idx) => &self.stack[idx],
            None => &NONE, // Should never happen with valid bytecode
        }
    }

    pub fn peek_mut(&mut self, distance: usize) -> &mut Value {
        let idx = self.stack.len() - 1 - distance;
        &mut self.stack[idx]
    }

    pub fn current_frame(&self) -> &CallFrame {
        self.frames.last().unwrap()
    }

    pub fn current_frame_mut(&mut self) -> &mut CallFrame {
        self.frames.last_mut().unwrap()
    }

    /// Collect all GC references held by this fiber (for garbage collection).
    pub fn gc_roots(&self) -> Vec<GcRef> {
        let mut roots = Vec::new();
        for val in &self.stack {
            if let Value::Object(r) = val {
                roots.push(*r);
            }
        }
        for frame in &self.frames {
            roots.push(frame.closure);
        }
        for &uv in &self.open_upvalues {
            roots.push(uv);
        }
        roots
    }
}
