use crate::fiber::FiberId;
use crate::object::PendingToken;
use crate::value::Value;
use std::collections::HashMap;

/// Type alias for fiber handles in the debug interface.
pub type FiberHandle = FiberId;

/// Action returned by observer hooks to control VM execution.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum DebugAction {
    /// Continue execution normally.
    Continue,
    /// Pause execution (VM will call on_breakpoint in a loop until resumed).
    Pause,
    /// Execute one line then pause (single-step).
    Step,
    /// Abort execution with an error.
    Abort,
}

/// A local variable's debug info, recording its bytecode lifetime.
#[derive(Debug, Clone)]
pub struct LocalVarEntry {
    pub slot: u8,
    pub name: String,
    pub start_offset: usize, // bytecode offset where local becomes live
    pub end_offset: usize,   // bytecode offset where local goes out of scope
}

/// Local variable scope for the current call frame.
pub struct Scope {
    pub locals: HashMap<String, Value>,
    pub upvalues: HashMap<String, Value>,
    /// Fiber ID of the currently executing fiber.
    pub fiber_id: FiberId,
    /// Pre-serialized display strings for locals (observer can't access heap/interner).
    pub local_strings: HashMap<String, String>,
}

/// A single frame in the call stack, with resolved debug info.
pub struct StackFrame {
    pub name: String,
    pub line: usize,
    pub scope: Scope,
}

/// Full VM state snapshot for breakpoint inspection.
pub struct VmState {
    pub line: usize,
    pub call_stack: Vec<StackFrame>,
    pub globals: HashMap<String, Value>,
    pub fiber_id: FiberId,
    pub source_line: String,
    /// Pre-serialized display strings for globals (observer can't access heap/interner).
    pub global_strings: HashMap<String, String>,
}

/// Trait for observing VM execution. All methods have default no-op implementations
/// so observers only need to override the hooks they care about.
pub trait VmObserver {
    /// Called when execution moves to a new source line.
    fn on_line(&mut self, line: usize, scope: &Scope) -> DebugAction {
        let _ = (line, scope);
        DebugAction::Continue
    }

    /// Called when a breakpoint is hit, or while paused waiting for resume.
    fn on_breakpoint(&mut self, line: usize, state: &VmState) -> DebugAction {
        let _ = (line, state);
        DebugAction::Continue
    }

    /// Called before a native (substrate) function is invoked.
    fn on_substrate_call(&mut self, name: &str, args: &[Value]) -> DebugAction {
        let _ = (name, args);
        DebugAction::Continue
    }

    /// Called after a native (substrate) function returns.
    fn on_substrate_return(&mut self, name: &str, result: &Value) {
        let _ = (name, result);
    }

    /// Called when a fiber parks on a pending native call.
    fn on_fiber_park(&mut self, fiber_id: FiberId, token: PendingToken) {
        let _ = (fiber_id, token);
    }

    /// Called when a parked fiber is resumed with a value.
    fn on_fiber_resume(&mut self, fiber_id: FiberId) {
        let _ = fiber_id;
    }

    /// Called when a fiber is cancelled.
    /// If the fiber was parked, `token` contains the pending operation token
    /// so the host can clean up associated I/O.
    fn on_fiber_cancel(&mut self, fiber_id: FiberId, token: Option<PendingToken>) {
        let _ = (fiber_id, token);
    }
}
