use std::collections::{HashMap, HashSet, VecDeque};

use crate::chunk::OpCode;
use crate::debug::{DebugAction, Scope, StackFrame, VmObserver, VmState};
use crate::fiber::{CallFrame, Fiber, FiberId, FiberState};
use crate::gc::{GcHeap, GcRef};
use crate::intern::{InternId, InternTable};
use crate::native;
use crate::object::*;
use crate::stdlib;
use crate::value::Value;
use ni_error::NiError;

const MAX_INSTRUCTIONS: usize = 1_000_000;
const MAX_FRAMES: usize = 256;
/// Maximum single allocation size (100 MB) for string repeat, Bytes(), etc.
const MAX_ALLOC_SIZE: usize = 100 * 1024 * 1024;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum FailPolicy {
    Error, // return Err(NiError) -- current behavior, good for tests
    Log,   // push to vm.output, return Ok -- good for game hosts
}

/// Configuration for creating a VM instance.
#[derive(Debug, Clone)]
pub struct VmConfig {
    /// Maximum heap bytes before a runtime error is raised. None = unlimited.
    pub memory_limit: Option<usize>,
    pub instruction_limit: usize,
    pub gc_threshold: usize,
    pub enable_specs: bool,
    /// Allowlist of native module names (math, random, time, json, nion).
    /// None means all modules are available.
    pub allowed_modules: Option<Vec<String>>,
    /// Global instruction limit across all fibers. None = instruction_limit * 10.
    pub global_instruction_limit: Option<u64>,
}

impl Default for VmConfig {
    fn default() -> Self {
        Self {
            memory_limit: None,
            instruction_limit: MAX_INSTRUCTIONS,
            gc_threshold: 256,
            enable_specs: false,
            allowed_modules: None,
            global_instruction_limit: None,
        }
    }
}

/// Status of the VM after a `run_ready()` call.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum VmStatus {
    /// All fibers have finished executing.
    AllDone,
    /// All remaining fibers are parked (waiting on async I/O).
    AllParked,
    /// Some fibers are suspended (yield/wait) -- call run_ready() again.
    Suspended,
    /// Some fibers are ready and some are parked.
    Mixed,
}

/// Information about a native (substrate) call that returned Pending.
#[derive(Debug, Clone)]
pub struct SubstrateCall {
    pub name: String,
    pub args: Vec<Value>,
    pub fiber_id: FiberId,
    pub component: String,
}

/// Trait for receiving notifications about async I/O events.
pub trait IoProvider {
    /// Called when a fiber parks on a pending native call.
    fn on_pending(&mut self, token: PendingToken, call: SubstrateCall);

    /// Called before a fiber begins executing in run_ready().
    /// Allows the host to swap per-fiber context (e.g. thread-local params).
    fn on_fiber_start(&mut self, _fiber_id: FiberId) {}

    /// Called when a pending operation is cancelled (its fiber was cancelled).
    /// The host should clean up any in-flight I/O associated with the token.
    fn on_cancel(&mut self, _token: PendingToken) {}
}

/// Runtime statistics snapshot.
#[derive(Debug, Clone)]
pub struct VmStats {
    pub memory_used: usize,
    pub objects_live: usize,
    pub instructions_executed: u64,
    pub bytes_allocated: usize,
}

pub struct Vm {
    pub heap: GcHeap,
    pub globals: HashMap<InternId, Value>,
    pub interner: InternTable,
    pub fiber: Fiber,
    pub output: Vec<String>,
    pub suppress_print: bool,
    pub fail_policy: FailPolicy,
    init_id: InternId,
    instruction_limit: usize,
    gc_enabled: bool,
    memory_limit: Option<usize>,
    observer: Option<Box<dyn VmObserver>>,
    breakpoints: HashSet<usize>,
    stepping: bool,
    last_debug_line: usize,
    // Async/fiber scheduling
    ready_fibers: VecDeque<(FiberId, Fiber)>,
    suspended_fibers: VecDeque<(FiberId, Fiber)>,
    parked_fibers: HashMap<PendingToken, (FiberId, Fiber)>,
    finished_fibers: Vec<(FiberId, Value)>,
    current_fiber_id: FiberId,
    io_provider: Option<Box<dyn IoProvider>>,
    total_instructions: u64,
    global_instruction_limit: Option<u64>,
    /// Stores pending info from the last native call that returned Pending.
    /// (token, function_name, args)
    last_pending: Option<(PendingToken, String, Vec<Value>)>,
    /// Extra GC roots for host-pinned objects (e.g. event handler closures).
    /// The host adds GcRefs here to prevent GC from collecting objects
    /// that are referenced outside the VM's normal root set.
    pub extra_roots: Vec<GcRef>,
}

/// Internal result from running a single fiber.
enum RunResult {
    Finished(Value),
    Parked(PendingToken, SubstrateCall),
    Suspended,
}

impl Default for Vm {
    fn default() -> Self {
        Self::new()
    }
}

impl Vm {
    pub fn new() -> Self {
        Self::with_config(VmConfig::default())
    }

    pub fn with_config(config: VmConfig) -> Self {
        let mut interner = InternTable::new();
        let init_id = interner.intern("init");
        let mut heap = GcHeap::new();
        heap.set_threshold(config.gc_threshold);
        let mut vm = Self {
            heap,
            globals: HashMap::new(),
            interner,
            fiber: Fiber::empty(),
            output: Vec::new(),
            suppress_print: false,
            fail_policy: FailPolicy::Error,
            init_id,
            instruction_limit: config.instruction_limit,
            gc_enabled: true,
            memory_limit: config.memory_limit,
            observer: None,
            breakpoints: HashSet::new(),
            stepping: false,
            last_debug_line: 0,
            ready_fibers: VecDeque::new(),
            suspended_fibers: VecDeque::new(),
            parked_fibers: HashMap::new(),
            finished_fibers: Vec::new(),
            current_fiber_id: FiberId::next(),
            io_provider: None,
            total_instructions: 0,
            global_instruction_limit: config.global_instruction_limit,
            last_pending: None,
            extra_roots: Vec::new(),
        };
        vm.define_natives();
        vm
    }

    fn define_natives(&mut self) {
        for native_fn in native::all_natives() {
            let id = self.interner.intern(&native_fn.name);
            let r = self.heap.alloc(NiObject::NativeFunction(native_fn));
            self.globals.insert(id, Value::Object(r));
        }
    }

    /// Register a native Rust function callable from Ni scripts.
    pub fn register_native(
        &mut self,
        name: &str,
        arity: i8,
        f: fn(&[Value], &mut GcHeap, &InternTable) -> NativeResult,
    ) {
        let native_fn = NativeFn {
            name: name.to_string(),
            arity,
            function: f,
        };
        let id = self.interner.intern(name);
        let r = self.heap.alloc(NiObject::NativeFunction(native_fn));
        self.globals.insert(id, Value::Object(r));
    }

    /// Get a global value by name.
    pub fn get_global(&self, name: &str) -> Option<Value> {
        let id = self.interner.find(name)?;
        self.globals.get(&id).cloned()
    }

    /// Call a Ni callable (closure, bound method, etc.) with arguments from Rust.
    pub fn call(&mut self, callable: Value, args: &[Value]) -> Result<Value, NiError> {
        // Push callable as slot 0 (will be replaced by receiver for classes)
        self.fiber.push(callable.clone());
        // Push arguments
        for arg in args {
            self.fiber.push(arg.clone());
        }
        self.call_value(args.len())?;
        self.run()
    }

    /// Set the per-interpret instruction limit.
    pub fn set_instruction_limit(&mut self, limit: usize) {
        self.instruction_limit = limit;
    }

    /// Force a garbage collection cycle.
    pub fn gc_collect(&mut self) {
        self.collect_garbage();
    }

    /// Disable automatic garbage collection.
    pub fn gc_disable(&mut self) {
        self.gc_enabled = false;
    }

    /// Enable automatic garbage collection.
    pub fn gc_enable(&mut self) {
        self.gc_enabled = true;
    }

    /// Set the GC collection threshold (object count that triggers collection).
    pub fn gc_set_threshold(&mut self, n: usize) {
        self.heap.set_threshold(n);
    }

    /// Set the GC growth factor (multiplier for threshold after collection).
    pub fn gc_set_growth_factor(&mut self, f: f64) {
        self.heap.set_growth_factor(f);
    }

    /// Set a memory limit (maximum heap bytes allocated). None = unlimited.
    pub fn set_memory_limit(&mut self, limit: Option<usize>) {
        self.memory_limit = limit;
    }

    /// Set the global instruction limit across all fibers. None = instruction_limit * 10.
    pub fn set_global_instruction_limit(&mut self, limit: Option<u64>) {
        self.global_instruction_limit = limit;
    }

    /// Set the IoProvider for receiving async I/O notifications.
    pub fn set_io_provider(&mut self, provider: Box<dyn IoProvider>) {
        self.io_provider = Some(provider);
    }

    /// Return a snapshot of runtime statistics.
    pub fn stats(&self) -> VmStats {
        VmStats {
            memory_used: self.heap.bytes_allocated(),
            objects_live: self.heap.object_count(),
            instructions_executed: self.total_instructions,
            bytes_allocated: self.heap.bytes_allocated(),
        }
    }

    /// Attach a debugger observer. Returns the previous observer if one was attached.
    pub fn attach_debugger(
        &mut self,
        observer: Box<dyn VmObserver>,
    ) -> Option<Box<dyn VmObserver>> {
        self.stepping = false;
        self.last_debug_line = 0;
        self.observer.replace(observer)
    }

    /// Detach the current debugger observer, returning it.
    pub fn detach_debugger(&mut self) -> Option<Box<dyn VmObserver>> {
        self.stepping = false;
        self.last_debug_line = 0;
        self.observer.take()
    }

    /// Set a breakpoint at a source line.
    pub fn set_breakpoint(&mut self, line: usize) {
        self.breakpoints.insert(line);
    }

    /// Clear a breakpoint at a source line.
    pub fn clear_breakpoint(&mut self, line: usize) {
        self.breakpoints.remove(&line);
    }

    /// Clear all breakpoints.
    pub fn clear_all_breakpoints(&mut self) {
        self.breakpoints.clear();
    }

    /// Return a sorted list of all current breakpoint lines.
    pub fn breakpoints(&self) -> Vec<usize> {
        let mut lines: Vec<usize> = self.breakpoints.iter().copied().collect();
        lines.sort();
        lines
    }

    /// Register a native class callable from Ni scripts.
    pub fn register_class(&mut self, name: &str) -> NativeClassBuilder<'_> {
        NativeClassBuilder {
            vm: self,
            name: name.to_string(),
            methods: HashMap::new(),
            static_methods: HashMap::new(),
        }
    }

    pub fn interpret(&mut self, closure_ref: GcRef) -> Result<Value, NiError> {
        self.fiber = Fiber::new(closure_ref);
        self.fiber.state = FiberState::Running;
        self.run()
    }

    /// Load a closure into the main fiber for execution via `run_ready()`.
    pub fn load(&mut self, closure_ref: GcRef) -> Result<(), NiError> {
        self.fiber = Fiber::new(closure_ref);
        self.fiber.state = FiberState::Created;
        self.current_fiber_id = FiberId::next();
        Ok(())
    }

    /// Run all ready fibers. Returns the VM status after the run.
    ///
    /// `delta_time` is the elapsed time in seconds since the last call.
    /// Suspended fibers with wait timers are decremented by delta_time;
    /// those whose timer expires are promoted back to ready. Yielded fibers
    /// (wait_timer == 0) are promoted to ready immediately.
    ///
    /// - Runs the main fiber first if it has frames.
    /// - Then drains the ready_fibers queue.
    /// - Fibers that return Parked are moved to parked_fibers.
    /// - Fibers that suspend are moved to suspended_fibers.
    /// - Fibers that finish are moved to finished_fibers.
    pub fn run_ready(&mut self, delta_time: f64) -> Result<VmStatus, NiError> {
        // Process suspended fibers: decrement wait timers, promote ready ones
        let mut still_suspended = VecDeque::new();
        while let Some((fid, mut fiber)) = self.suspended_fibers.pop_front() {
            if fiber.wait_timer > 0.0 {
                fiber.wait_timer -= delta_time;
                if fiber.wait_timer <= 0.0 {
                    fiber.wait_timer = 0.0;
                    fiber.state = FiberState::Created;
                    self.ready_fibers.push_back((fid, fiber));
                } else {
                    still_suspended.push_back((fid, fiber));
                }
            } else {
                // Yielded fiber (no timer) -- ready immediately
                fiber.state = FiberState::Created;
                self.ready_fibers.push_back((fid, fiber));
            }
        }
        self.suspended_fibers = still_suspended;

        // Run main fiber if it's ready
        if !self.fiber.frames.is_empty()
            && self.fiber.state != FiberState::Parked
            && self.fiber.state != FiberState::Suspended
        {
            if let Some(ref mut io) = self.io_provider {
                io.on_fiber_start(self.current_fiber_id);
            }
            self.fiber.state = FiberState::Running;
            match self.run_fiber()? {
                RunResult::Finished(val) => {
                    self.fiber.state = FiberState::Finished;
                    self.finished_fibers.push((self.current_fiber_id, val));
                }
                RunResult::Parked(token, call) => {
                    self.fiber.state = FiberState::Parked;
                    let parked_id = self.current_fiber_id;
                    if let Some(ref mut obs) = self.observer {
                        obs.on_fiber_park(parked_id, token);
                    }
                    if let Some(ref mut io) = self.io_provider {
                        io.on_pending(token, call);
                    }
                    let parked_fiber = std::mem::replace(&mut self.fiber, Fiber::empty());
                    self.parked_fibers.insert(token, (parked_id, parked_fiber));
                }
                RunResult::Suspended => {
                    // fiber.state already set to Suspended by the opcode handler
                    let suspended_fiber = std::mem::replace(&mut self.fiber, Fiber::empty());
                    self.suspended_fibers
                        .push_back((self.current_fiber_id, suspended_fiber));
                }
            }
        }

        // Drain ready_fibers queue
        while let Some((fid, fiber)) = self.ready_fibers.pop_front() {
            let saved_main = std::mem::replace(&mut self.fiber, fiber);
            let saved_id = self.current_fiber_id;
            self.current_fiber_id = fid;
            if let Some(ref mut io) = self.io_provider {
                io.on_fiber_start(fid);
            }
            self.fiber.state = FiberState::Running;

            match self.run_fiber()? {
                RunResult::Finished(val) => {
                    self.fiber.state = FiberState::Finished;
                    self.finished_fibers.push((fid, val));
                }
                RunResult::Parked(token, call) => {
                    self.fiber.state = FiberState::Parked;
                    if let Some(ref mut obs) = self.observer {
                        obs.on_fiber_park(fid, token);
                    }
                    if let Some(ref mut io) = self.io_provider {
                        io.on_pending(token, call);
                    }
                    let parked_fiber = std::mem::replace(&mut self.fiber, Fiber::empty());
                    self.parked_fibers.insert(token, (fid, parked_fiber));
                }
                RunResult::Suspended => {
                    let suspended_fiber = std::mem::replace(&mut self.fiber, Fiber::empty());
                    self.suspended_fibers.push_back((fid, suspended_fiber));
                }
            }

            // Restore main fiber
            self.fiber = saved_main;
            self.current_fiber_id = saved_id;
        }

        // Determine status
        let has_parked = !self.parked_fibers.is_empty();
        let has_suspended = !self.suspended_fibers.is_empty();
        let main_active = !self.fiber.frames.is_empty() && self.fiber.state != FiberState::Finished;

        if !has_parked && !has_suspended && !main_active {
            Ok(VmStatus::AllDone)
        } else if has_suspended {
            Ok(VmStatus::Suspended)
        } else if has_parked && !has_suspended && !main_active {
            Ok(VmStatus::AllParked)
        } else {
            Ok(VmStatus::Mixed)
        }
    }

    /// Resume a parked fiber with a resolved value.
    pub fn resume(&mut self, token: PendingToken, value: Value) -> Result<(), NiError> {
        let (fid, mut fiber) = self
            .parked_fibers
            .remove(&token)
            .ok_or_else(|| NiError::runtime("No parked fiber for this token"))?;
        // Pop the None placeholder that was pushed when the native returned Pending
        fiber.pop();
        // Push the resolved value
        fiber.push(value);
        fiber.state = FiberState::Created; // ready to run
        if let Some(ref mut obs) = self.observer {
            obs.on_fiber_resume(fid);
        }
        self.ready_fibers.push_back((fid, fiber));
        Ok(())
    }

    /// Return the list of finished fiber results.
    pub fn finished_fibers(&self) -> &[(FiberId, Value)] {
        &self.finished_fibers
    }

    /// Return the number of parked fibers.
    pub fn parked_count(&self) -> usize {
        self.parked_fibers.len()
    }

    /// Return the number of suspended fibers (yield/wait).
    pub fn suspended_count(&self) -> usize {
        self.suspended_fibers.len()
    }

    /// Return the current fiber ID.
    pub fn current_fiber_id(&self) -> FiberId {
        self.current_fiber_id
    }

    /// Queue a compiled closure as a new fiber for execution via `run_ready()`.
    /// Returns the fiber ID assigned to the new fiber.
    pub fn spawn_closure(&mut self, closure: GcRef) -> FiberId {
        let fid = FiberId::next();
        self.ready_fibers.push_back((fid, Fiber::new(closure)));
        fid
    }

    /// Spawn a new fiber that calls a closure with arguments.
    /// The closure's arity must match args.len(). The fiber is
    /// queued in ready_fibers and will execute on the next run_ready().
    pub fn spawn_call(&mut self, closure: GcRef, args: &[Value]) -> FiberId {
        let mut fiber = Fiber::new(closure);
        for arg in args {
            fiber.stack.push(arg.clone());
        }
        let fid = FiberId::next();
        self.ready_fibers.push_back((fid, fiber));
        fid
    }

    /// Cancel a fiber by ID. Removes from ready or parked queues.
    /// Returns true if the fiber was found and cancelled.
    ///
    /// If the fiber was parked on async I/O, notifies the IoProvider so
    /// the host can clean up the in-flight operation, and fires the
    /// observer's `on_fiber_cancel` hook with the associated token.
    pub fn cancel_fiber(&mut self, fid: FiberId) -> bool {
        // Check ready queue
        if let Some(pos) = self.ready_fibers.iter().position(|(id, _)| *id == fid) {
            let (_, mut fiber) = self.ready_fibers.remove(pos).unwrap();
            fiber.state = FiberState::Cancelled;
            if let Some(ref mut obs) = self.observer {
                obs.on_fiber_cancel(fid, None);
            }
            return true;
        }
        // Check suspended fibers
        if let Some(pos) = self.suspended_fibers.iter().position(|(id, _)| *id == fid) {
            let (_, mut fiber) = self.suspended_fibers.remove(pos).unwrap();
            fiber.state = FiberState::Cancelled;
            if let Some(ref mut obs) = self.observer {
                obs.on_fiber_cancel(fid, None);
            }
            return true;
        }
        // Check parked fibers (keyed by PendingToken, search by FiberId)
        let token = self
            .parked_fibers
            .iter()
            .find(|(_, (id, _))| *id == fid)
            .map(|(token, _)| *token);
        if let Some(token) = token {
            let (_, mut fiber) = self.parked_fibers.remove(&token).unwrap();
            fiber.state = FiberState::Cancelled;
            if let Some(ref mut io) = self.io_provider {
                io.on_cancel(token);
            }
            if let Some(ref mut obs) = self.observer {
                obs.on_fiber_cancel(fid, Some(token));
            }
            return true;
        }
        false
    }

    /// Resolve a string from an object reference (handles both String and InternedString)
    fn resolve_string_content(&self, r: GcRef) -> Option<&str> {
        self.heap
            .get(r)
            .and_then(|o| o.as_string_with_intern(&self.interner))
    }

    /// Get the InternId from a constant pool entry (zero-allocation for InternedString)
    fn get_constant_intern_id(
        &self,
        chunk: &crate::chunk::Chunk,
        idx: u16,
    ) -> Result<InternId, NiError> {
        match &chunk.constants[idx as usize] {
            Value::Object(r) => {
                let obj = self
                    .heap
                    .get(*r)
                    .ok_or_else(|| NiError::runtime("Expected string constant"))?;
                match obj {
                    NiObject::InternedString(id) => Ok(*id),
                    NiObject::String(s) => {
                        // Fallback: look up in interner (shouldn't happen in normal flow)
                        self.interner
                            .find(s)
                            .ok_or_else(|| NiError::runtime("String not interned"))
                    }
                    _ => Err(NiError::runtime("Expected string constant")),
                }
            }
            _ => Err(NiError::runtime("Expected string constant")),
        }
    }

    /// Build a Scope from the current call frame using the local variable table and upvalue names.
    fn build_scope(&self, frame: &CallFrame) -> Scope {
        let mut locals = HashMap::new();
        let mut upvalues = HashMap::new();
        let mut local_strings = HashMap::new();
        if let Some(obj) = self.heap.get(frame.closure) {
            if let Some(closure) = obj.as_closure() {
                if let Some(fn_obj) = self.heap.get(closure.function) {
                    if let Some(func) = fn_obj.as_function() {
                        // Locals: filter by IP range for accurate nested-block visibility
                        for entry in &func.local_var_table {
                            if entry.name.is_empty() {
                                continue; // skip slot 0 (the function itself)
                            }
                            if entry.start_offset <= frame.ip && frame.ip < entry.end_offset {
                                let stack_idx = frame.stack_base + entry.slot as usize;
                                if stack_idx < self.fiber.stack.len() {
                                    let val = self.fiber.stack[stack_idx].clone();
                                    local_strings.insert(
                                        entry.name.clone(),
                                        native::value_to_display_string(
                                            &val,
                                            &self.heap,
                                            &self.interner,
                                        ),
                                    );
                                    locals.insert(entry.name.clone(), val);
                                }
                            }
                        }
                        // Upvalues: pair closure.upvalues with func.upvalue_names
                        for (i, uv_name) in func.upvalue_names.iter().enumerate() {
                            if i < closure.upvalues.len() {
                                if let Ok(val) = self.read_upvalue(closure.upvalues[i]) {
                                    upvalues.insert(uv_name.clone(), val);
                                }
                            }
                        }
                    }
                }
            }
        }
        Scope {
            locals,
            upvalues,
            fiber_id: self.current_fiber_id,
            local_strings,
        }
    }

    /// Build a full VmState snapshot for breakpoint inspection.
    fn build_vm_state(&self, current_line: usize) -> VmState {
        let mut call_stack = Vec::new();
        for (i, frame) in self.fiber.frames.iter().enumerate().rev() {
            let (name, line) = if let Some(obj) = self.heap.get(frame.closure) {
                if let Some(closure) = obj.as_closure() {
                    if let Some(fn_obj) = self.heap.get(closure.function) {
                        if let Some(func) = fn_obj.as_function() {
                            let line = if i == self.fiber.frames.len() - 1 {
                                current_line
                            } else {
                                // For non-top frames, use the line at ip-1 (the call site)
                                let ip = frame.ip.saturating_sub(1);
                                if ip < func.chunk.lines.len() {
                                    func.chunk.lines[ip]
                                } else {
                                    0
                                }
                            };
                            (func.name.clone(), line)
                        } else {
                            ("<unknown>".to_string(), 0)
                        }
                    } else {
                        ("<unknown>".to_string(), 0)
                    }
                } else {
                    ("<unknown>".to_string(), 0)
                }
            } else {
                ("<unknown>".to_string(), 0)
            };
            let scope = self.build_scope(frame);
            call_stack.push(StackFrame { name, line, scope });
        }
        // Collect globals with resolved names and display strings
        let mut globals = HashMap::new();
        let mut global_strings = HashMap::new();
        for (&id, value) in &self.globals {
            let name = self.interner.resolve(id).to_string();
            global_strings.insert(
                name.clone(),
                native::value_to_display_string(value, &self.heap, &self.interner),
            );
            globals.insert(name, value.clone());
        }

        // Resolve source_line from the top frame's function chunk
        let source_line = self.resolve_source_line(current_line);

        VmState {
            line: current_line,
            call_stack,
            globals,
            fiber_id: self.current_fiber_id,
            source_line,
            global_strings,
        }
    }

    /// Resolve the source text for a given line number.
    /// Currently returns empty -- source text is not stored in compiled chunks.
    fn resolve_source_line(&self, _line: usize) -> String {
        String::new()
    }

    /// Handle a DebugAction returned by an observer hook.
    /// Returns Ok(true) if execution should continue, Err on abort.
    fn handle_debug_action(
        &mut self,
        action: DebugAction,
        current_line: usize,
    ) -> Result<(), NiError> {
        match action {
            DebugAction::Continue => {
                self.stepping = false;
            }
            DebugAction::Step => {
                self.stepping = true;
            }
            DebugAction::Pause => {
                // Spin calling on_breakpoint until the observer says to continue or step
                loop {
                    let state = self.build_vm_state(current_line);
                    let action = if let Some(ref mut obs) = self.observer {
                        obs.on_breakpoint(current_line, &state)
                    } else {
                        DebugAction::Continue
                    };
                    match action {
                        DebugAction::Continue => {
                            self.stepping = false;
                            break;
                        }
                        DebugAction::Step => {
                            self.stepping = true;
                            break;
                        }
                        DebugAction::Abort => {
                            return Err(NiError::runtime("Execution aborted by debugger"));
                        }
                        DebugAction::Pause => {
                            std::thread::yield_now();
                            continue;
                        }
                    }
                }
            }
            DebugAction::Abort => {
                return Err(NiError::runtime("Execution aborted by debugger"));
            }
        }
        Ok(())
    }

    fn run(&mut self) -> Result<Value, NiError> {
        match self.run_fiber()? {
            RunResult::Finished(val) => Ok(val),
            RunResult::Parked(_, _) => Err(NiError::runtime(
                "Async native called without await -- use run_ready()",
            )),
            RunResult::Suspended => {
                // In synchronous mode, yield/wait just returns None
                Ok(Value::None)
            }
        }
    }

    fn run_fiber(&mut self) -> Result<RunResult, NiError> {
        let mut instruction_count = 0usize;

        loop {
            if instruction_count >= self.instruction_limit {
                return Err(NiError::runtime(
                    "Instruction limit exceeded (possible infinite loop)",
                ));
            }
            instruction_count += 1;
            self.total_instructions += 1;
            // Global instruction limit across all fibers
            let global_limit = self
                .global_instruction_limit
                .unwrap_or((self.instruction_limit as u64) * 10);
            if self.total_instructions > global_limit {
                return Err(NiError::runtime(
                    "Global instruction limit exceeded across fibers",
                ));
            }

            if self.fiber.frames.is_empty() {
                return Ok(RunResult::Finished(Value::None));
            }

            let frame_idx = self.fiber.frames.len() - 1;
            let closure_ref = self.fiber.frames[frame_idx].closure;
            let ip = self.fiber.frames[frame_idx].ip;

            // Get the function's chunk
            let (fn_ref, upvalue_refs) = {
                let closure_obj = self
                    .heap
                    .get(closure_ref)
                    .ok_or_else(|| NiError::runtime("Invalid closure reference"))?;
                match closure_obj {
                    NiObject::Closure(c) => (c.function, c.upvalues.clone()),
                    _ => return Err(NiError::runtime("Expected closure")),
                }
            };

            let chunk = {
                let fn_obj = self
                    .heap
                    .get(fn_ref)
                    .ok_or_else(|| NiError::runtime("Invalid function reference"))?;
                match fn_obj {
                    NiObject::Function(f) => f.chunk.clone(),
                    _ => return Err(NiError::runtime("Expected function")),
                }
            };

            if ip >= chunk.code.len() {
                // Function ended without return
                let stack_base = self.fiber.frames[frame_idx].stack_base;
                self.fiber.frames.pop();
                self.fiber.stack.truncate(stack_base);
                self.fiber.push(Value::None);
                if self.fiber.frames.is_empty() {
                    return Ok(RunResult::Finished(self.fiber.pop()));
                }
                continue;
            }

            let op_byte = chunk.code[ip];
            let current_line = chunk.lines[ip];
            self.fiber.frames[frame_idx].ip = ip + 1;

            // Debug hooks -- only fire when an observer is attached and line changes
            if self.observer.is_some() && current_line != self.last_debug_line {
                self.last_debug_line = current_line;

                if self.breakpoints.contains(&current_line) {
                    let state = self.build_vm_state(current_line);
                    let action = self
                        .observer
                        .as_mut()
                        .unwrap()
                        .on_breakpoint(current_line, &state);
                    self.handle_debug_action(action, current_line)?;
                } else if self.stepping {
                    let frame = &self.fiber.frames[frame_idx];
                    let scope = self.build_scope(frame);
                    let action = self
                        .observer
                        .as_mut()
                        .unwrap()
                        .on_line(current_line, &scope);
                    self.handle_debug_action(action, current_line)?;
                } else {
                    let frame = &self.fiber.frames[frame_idx];
                    let scope = self.build_scope(frame);
                    let action = self
                        .observer
                        .as_mut()
                        .unwrap()
                        .on_line(current_line, &scope);
                    self.handle_debug_action(action, current_line)?;
                }
            }

            let op = match op_byte {
                x if x == OpCode::Constant as u8 => OpCode::Constant,
                x if x == OpCode::None as u8 => OpCode::None,
                x if x == OpCode::True as u8 => OpCode::True,
                x if x == OpCode::False as u8 => OpCode::False,
                x if x == OpCode::Pop as u8 => OpCode::Pop,
                x if x == OpCode::Dup as u8 => OpCode::Dup,
                x if x == OpCode::GetLocal as u8 => OpCode::GetLocal,
                x if x == OpCode::SetLocal as u8 => OpCode::SetLocal,
                x if x == OpCode::GetGlobal as u8 => OpCode::GetGlobal,
                x if x == OpCode::SetGlobal as u8 => OpCode::SetGlobal,
                x if x == OpCode::DefineGlobal as u8 => OpCode::DefineGlobal,
                x if x == OpCode::GetUpvalue as u8 => OpCode::GetUpvalue,
                x if x == OpCode::SetUpvalue as u8 => OpCode::SetUpvalue,
                x if x == OpCode::CloseUpvalue as u8 => OpCode::CloseUpvalue,
                x if x == OpCode::Add as u8 => OpCode::Add,
                x if x == OpCode::Subtract as u8 => OpCode::Subtract,
                x if x == OpCode::Multiply as u8 => OpCode::Multiply,
                x if x == OpCode::Divide as u8 => OpCode::Divide,
                x if x == OpCode::Modulo as u8 => OpCode::Modulo,
                x if x == OpCode::Negate as u8 => OpCode::Negate,
                x if x == OpCode::Equal as u8 => OpCode::Equal,
                x if x == OpCode::NotEqual as u8 => OpCode::NotEqual,
                x if x == OpCode::Less as u8 => OpCode::Less,
                x if x == OpCode::Greater as u8 => OpCode::Greater,
                x if x == OpCode::LessEqual as u8 => OpCode::LessEqual,
                x if x == OpCode::GreaterEqual as u8 => OpCode::GreaterEqual,
                x if x == OpCode::Not as u8 => OpCode::Not,
                x if x == OpCode::Jump as u8 => OpCode::Jump,
                x if x == OpCode::JumpIfFalse as u8 => OpCode::JumpIfFalse,
                x if x == OpCode::JumpIfTrue as u8 => OpCode::JumpIfTrue,
                x if x == OpCode::Loop as u8 => OpCode::Loop,
                x if x == OpCode::Call as u8 => OpCode::Call,
                x if x == OpCode::Return as u8 => OpCode::Return,
                x if x == OpCode::Closure as u8 => OpCode::Closure,
                x if x == OpCode::GetProperty as u8 => OpCode::GetProperty,
                x if x == OpCode::SetProperty as u8 => OpCode::SetProperty,
                x if x == OpCode::GetIndex as u8 => OpCode::GetIndex,
                x if x == OpCode::SetIndex as u8 => OpCode::SetIndex,
                x if x == OpCode::Class as u8 => OpCode::Class,
                x if x == OpCode::Method as u8 => OpCode::Method,
                x if x == OpCode::Inherit as u8 => OpCode::Inherit,
                x if x == OpCode::GetSuper as u8 => OpCode::GetSuper,
                x if x == OpCode::Invoke as u8 => OpCode::Invoke,
                x if x == OpCode::SuperInvoke as u8 => OpCode::SuperInvoke,
                x if x == OpCode::BuildList as u8 => OpCode::BuildList,
                x if x == OpCode::BuildMap as u8 => OpCode::BuildMap,
                x if x == OpCode::BuildRange as u8 => OpCode::BuildRange,
                x if x == OpCode::GetIterator as u8 => OpCode::GetIterator,
                x if x == OpCode::IteratorNext as u8 => OpCode::IteratorNext,
                x if x == OpCode::SpawnFiber as u8 => OpCode::SpawnFiber,
                x if x == OpCode::Yield as u8 => OpCode::Yield,
                x if x == OpCode::Wait as u8 => OpCode::Wait,
                x if x == OpCode::Await as u8 => OpCode::Await,
                x if x == OpCode::StringConcat as u8 => OpCode::StringConcat,
                x if x == OpCode::SafeNav as u8 => OpCode::SafeNav,
                x if x == OpCode::NoneCoalesce as u8 => OpCode::NoneCoalesce,
                x if x == OpCode::Fail as u8 => OpCode::Fail,
                x if x == OpCode::SetCatchPoint as u8 => OpCode::SetCatchPoint,
                x if x == OpCode::ClearCatchPoint as u8 => OpCode::ClearCatchPoint,
                x if x == OpCode::Print as u8 => OpCode::Print,
                x if x == OpCode::AssertOp as u8 => OpCode::AssertOp,
                x if x == OpCode::AssertCmp as u8 => OpCode::AssertCmp,
                x if x == OpCode::SetDocstring as u8 => OpCode::SetDocstring,
                _ => return Err(NiError::runtime(format!("Unknown opcode: {}", op_byte))),
            };

            match op {
                OpCode::Constant => {
                    let idx = self.read_u16(&chunk);
                    let val = chunk.constants[idx as usize].clone();
                    self.fiber.push(val);
                }
                OpCode::None => self.fiber.push(Value::None),
                OpCode::True => self.fiber.push(Value::Bool(true)),
                OpCode::False => self.fiber.push(Value::Bool(false)),
                OpCode::Pop => {
                    self.fiber.pop();
                }
                OpCode::Dup => {
                    let val = self.fiber.peek(0).clone();
                    self.fiber.push(val);
                }

                OpCode::GetLocal => {
                    let slot = self.read_byte(&chunk);
                    let stack_base = self.fiber.frames.last().unwrap().stack_base;
                    let idx = stack_base + slot as usize;
                    let val = self.fiber.stack.get(idx).cloned().unwrap_or(Value::None);
                    self.fiber.push(val);
                }
                OpCode::SetLocal => {
                    let slot = self.read_byte(&chunk);
                    let val = self.fiber.peek(0).clone();
                    let stack_base = self.fiber.frames.last().unwrap().stack_base;
                    let idx = stack_base + slot as usize;
                    if idx < self.fiber.stack.len() {
                        self.fiber.stack[idx] = val;
                    }
                }
                OpCode::GetGlobal => {
                    let idx = self.read_u16(&chunk);
                    let id = self.get_constant_intern_id(&chunk, idx)?;
                    let val = self.globals.get(&id).cloned().ok_or_else(|| {
                        NiError::runtime(format!(
                            "Undefined variable '{}'",
                            self.interner.resolve(id)
                        ))
                    })?;
                    self.fiber.push(val);
                }
                OpCode::SetGlobal => {
                    let idx = self.read_u16(&chunk);
                    let id = self.get_constant_intern_id(&chunk, idx)?;
                    if !self.globals.contains_key(&id) {
                        return Err(NiError::runtime(format!(
                            "Undefined variable '{}'",
                            self.interner.resolve(id)
                        )));
                    }
                    let val = self.fiber.peek(0).clone();
                    self.globals.insert(id, val);
                }
                OpCode::DefineGlobal => {
                    let idx = self.read_u16(&chunk);
                    let id = self.get_constant_intern_id(&chunk, idx)?;
                    let val = self.fiber.pop();
                    self.globals.insert(id, val);
                }

                OpCode::GetUpvalue => {
                    let slot = self.read_byte(&chunk) as usize;
                    if slot < upvalue_refs.len() {
                        let upvalue_ref = upvalue_refs[slot];
                        let val = self.read_upvalue(upvalue_ref)?;
                        self.fiber.push(val);
                    } else {
                        self.fiber.push(Value::None);
                    }
                }
                OpCode::SetUpvalue => {
                    let slot = self.read_byte(&chunk) as usize;
                    if slot < upvalue_refs.len() {
                        let upvalue_ref = upvalue_refs[slot];
                        let val = self.fiber.peek(0).clone();
                        self.write_upvalue(upvalue_ref, val)?;
                    }
                }
                OpCode::CloseUpvalue => {
                    let top = self.fiber.stack.len() - 1;
                    self.close_upvalues(top);
                    self.fiber.pop();
                }

                OpCode::Add => {
                    let b = self.fiber.pop();
                    let a = self.fiber.pop();
                    let result = self.op_add(a, b)?;
                    self.fiber.push(result);
                }
                OpCode::Subtract => {
                    let b = self.fiber.pop();
                    let a = self.fiber.pop();
                    let result = match (&a, &b) {
                        (Value::Int(a), Value::Int(b)) => a.checked_sub(*b).map(Value::Int).ok_or_else(|| NiError::runtime("Integer overflow"))?,
                        (Value::Float(a), Value::Float(b)) => Value::Float(a - b),
                        (Value::Int(a), Value::Float(b)) => Value::Float(*a as f64 - b),
                        (Value::Float(a), Value::Int(b)) => Value::Float(a - *b as f64),
                        _ => {
                            return Err(NiError::runtime(format!(
                                "Cannot subtract {} and {}",
                                a.type_name(),
                                b.type_name()
                            )))
                        }
                    };
                    self.fiber.push(result);
                }
                OpCode::Multiply => {
                    let b = self.fiber.pop();
                    let a = self.fiber.pop();
                    let result = self.op_multiply(a, b)?;
                    self.fiber.push(result);
                }
                OpCode::Divide => {
                    let b = self.fiber.pop();
                    let a = self.fiber.pop();
                    let result = match (&a, &b) {
                        (Value::Int(_), Value::Int(0)) | (Value::Float(_), Value::Int(0)) => {
                            return Err(NiError::runtime("Division by zero"))
                        }
                        (Value::Int(a), Value::Int(b)) => a.checked_div(*b).map(Value::Int).ok_or_else(|| NiError::runtime("Integer overflow"))?,
                        (Value::Float(a), Value::Float(b)) => Value::Float(a / b),
                        (Value::Int(a), Value::Float(b)) => Value::Float(*a as f64 / b),
                        (Value::Float(a), Value::Int(b)) => Value::Float(a / *b as f64),
                        _ => {
                            return Err(NiError::runtime(format!(
                                "Cannot divide {} and {}",
                                a.type_name(),
                                b.type_name()
                            )))
                        }
                    };
                    self.fiber.push(result);
                }
                OpCode::Modulo => {
                    let b = self.fiber.pop();
                    let a = self.fiber.pop();
                    let result = match (&a, &b) {
                        (Value::Int(a), Value::Int(b)) => {
                            if *b == 0 {
                                return Err(NiError::runtime("Modulo by zero"));
                            }
                            let rem = a.checked_rem(*b).ok_or_else(|| NiError::runtime("Integer overflow"))?;
                            let adjusted = rem.checked_add(*b).ok_or_else(|| NiError::runtime("Integer overflow"))?;
                            adjusted.checked_rem(*b).map(Value::Int).ok_or_else(|| NiError::runtime("Integer overflow"))?
                        }
                        (Value::Float(a), Value::Float(b)) => Value::Float(a % b),
                        (Value::Int(a), Value::Float(b)) => Value::Float(*a as f64 % b),
                        (Value::Float(a), Value::Int(b)) => Value::Float(a % *b as f64),
                        _ => return Err(NiError::runtime("Cannot modulo non-numbers")),
                    };
                    self.fiber.push(result);
                }
                OpCode::Negate => {
                    let val = self.fiber.pop();
                    let result = match val {
                        Value::Int(n) => n.checked_neg().map(Value::Int).ok_or_else(|| NiError::runtime("Integer overflow"))?,
                        Value::Float(f) => Value::Float(-f),
                        _ => return Err(NiError::runtime("Cannot negate non-number")),
                    };
                    self.fiber.push(result);
                }

                OpCode::Equal => {
                    let b = self.fiber.pop();
                    let a = self.fiber.pop();
                    let eq = self.values_equal(&a, &b);
                    self.fiber.push(Value::Bool(eq));
                }
                OpCode::NotEqual => {
                    let b = self.fiber.pop();
                    let a = self.fiber.pop();
                    let eq = self.values_equal(&a, &b);
                    self.fiber.push(Value::Bool(!eq));
                }
                OpCode::Less => {
                    let b = self.fiber.pop();
                    let a = self.fiber.pop();
                    let result =
                        self.compare_values(&a, &b, |ord| ord == std::cmp::Ordering::Less)?;
                    self.fiber.push(Value::Bool(result));
                }
                OpCode::Greater => {
                    let b = self.fiber.pop();
                    let a = self.fiber.pop();
                    let result =
                        self.compare_values(&a, &b, |ord| ord == std::cmp::Ordering::Greater)?;
                    self.fiber.push(Value::Bool(result));
                }
                OpCode::LessEqual => {
                    let b = self.fiber.pop();
                    let a = self.fiber.pop();
                    let result =
                        self.compare_values(&a, &b, |ord| ord != std::cmp::Ordering::Greater)?;
                    self.fiber.push(Value::Bool(result));
                }
                OpCode::GreaterEqual => {
                    let b = self.fiber.pop();
                    let a = self.fiber.pop();
                    let result =
                        self.compare_values(&a, &b, |ord| ord != std::cmp::Ordering::Less)?;
                    self.fiber.push(Value::Bool(result));
                }

                OpCode::Not => {
                    let val = self.fiber.pop();
                    let is_falsy = self.is_falsy(&val);
                    self.fiber.push(Value::Bool(is_falsy));
                }

                OpCode::Jump => {
                    let offset = self.read_u16(&chunk) as usize;
                    self.fiber.frames.last_mut().unwrap().ip += offset;
                }
                OpCode::JumpIfFalse => {
                    let offset = self.read_u16(&chunk) as usize;
                    if self.is_falsy(self.fiber.peek(0)) {
                        self.fiber.frames.last_mut().unwrap().ip += offset;
                    }
                }
                OpCode::JumpIfTrue => {
                    let offset = self.read_u16(&chunk) as usize;
                    if !self.is_falsy(self.fiber.peek(0)) {
                        self.fiber.frames.last_mut().unwrap().ip += offset;
                    }
                }
                OpCode::Loop => {
                    let offset = self.read_u16(&chunk) as usize;
                    self.fiber.frames.last_mut().unwrap().ip =
                        self.fiber.frames.last().unwrap().ip.saturating_sub(offset);
                }

                OpCode::Call => {
                    let arg_count = self.read_byte(&chunk) as usize;
                    self.call_value(arg_count)?;
                }
                OpCode::Return => {
                    let result = self.fiber.pop();
                    let frame = self.fiber.frames.pop().unwrap();

                    // Close upvalues for locals going out of scope
                    self.close_upvalues(frame.stack_base);

                    self.fiber.stack.truncate(frame.stack_base);

                    if self.fiber.frames.is_empty() {
                        return Ok(RunResult::Finished(result));
                    }
                    self.fiber.push(result);
                }
                OpCode::Closure => {
                    let fn_idx = self.read_u16(&chunk);
                    let fn_val = chunk.constants[fn_idx as usize].clone();
                    let fn_ref = fn_val.as_object().unwrap();

                    let upvalue_count = {
                        let f = self.heap.get(fn_ref).unwrap().as_function().unwrap();
                        f.upvalue_count as usize
                    };

                    let mut upvalues = Vec::with_capacity(upvalue_count);
                    for _ in 0..upvalue_count {
                        let is_local = self.read_byte(&chunk);
                        let index = self.read_byte(&chunk);
                        if is_local == 1 {
                            let stack_base = self.fiber.frames.last().unwrap().stack_base;
                            let abs_slot = stack_base + index as usize;
                            let uv = self.capture_upvalue(abs_slot);
                            upvalues.push(uv);
                        } else {
                            upvalues.push(upvalue_refs[index as usize]);
                        }
                    }

                    let closure = NiObject::Closure(NiClosure {
                        function: fn_ref,
                        upvalues,
                    });
                    let closure_ref = self.heap.alloc(closure);
                    self.fiber.push(Value::Object(closure_ref));
                }

                OpCode::GetProperty => {
                    let idx = self.read_u16(&chunk);
                    let id = self.get_constant_intern_id(&chunk, idx)?;
                    let receiver = self.fiber.pop();
                    let result = self.get_property(receiver, id)?;
                    self.fiber.push(result);
                }
                OpCode::SetProperty => {
                    let idx = self.read_u16(&chunk);
                    let id = self.get_constant_intern_id(&chunk, idx)?;
                    let value = self.fiber.pop();
                    let receiver = self.fiber.pop();
                    self.set_property(receiver, id, value.clone())?;
                    self.fiber.push(value);
                }
                OpCode::GetIndex => {
                    let index = self.fiber.pop();
                    let receiver = self.fiber.pop();
                    let result = self.get_index(receiver, index)?;
                    self.fiber.push(result);
                }
                OpCode::SetIndex => {
                    let value = self.fiber.pop();
                    let index = self.fiber.pop();
                    let receiver = self.fiber.pop();
                    self.set_index(receiver, index, value.clone())?;
                    self.fiber.push(value);
                }

                OpCode::Class => {
                    let idx = self.read_u16(&chunk);
                    let id = self.get_constant_intern_id(&chunk, idx)?;
                    let name = self.interner.resolve(id).to_string();
                    let class = NiObject::Class(NiClass {
                        name,
                        methods: HashMap::new(),
                        superclass: None,
                        fields: HashMap::new(),
                        docstring: None,
                    });
                    let r = self.heap.alloc(class);
                    self.fiber.push(Value::Object(r));
                }
                OpCode::Method => {
                    let idx = self.read_u16(&chunk);
                    let id = self.get_constant_intern_id(&chunk, idx)?;
                    let method = self.fiber.pop();
                    let class_val = self.fiber.peek(0).clone();
                    if let (Value::Object(class_ref), Value::Object(method_ref)) =
                        (class_val, method)
                    {
                        if let Some(class) = self.heap.get_mut(class_ref) {
                            if let Some(c) = class.as_class_mut() {
                                c.methods.insert(id, method_ref);
                            }
                        }
                    }
                }
                OpCode::Inherit => {
                    let superclass_val = self.fiber.pop();
                    let subclass_val = self.fiber.peek(0).clone();
                    if let (Value::Object(sub_ref), Value::Object(super_ref)) =
                        (subclass_val, superclass_val)
                    {
                        // Copy methods from superclass
                        let super_methods = {
                            let sc = self
                                .heap
                                .get(super_ref)
                                .ok_or_else(|| NiError::runtime("Invalid superclass"))?;
                            let sc = sc
                                .as_class()
                                .ok_or_else(|| NiError::runtime("Superclass must be a class"))?;
                            sc.methods.clone()
                        };
                        if let Some(sub_obj) = self.heap.get_mut(sub_ref) {
                            if let Some(sub_class) = sub_obj.as_class_mut() {
                                for (name, method) in super_methods {
                                    sub_class.methods.entry(name).or_insert(method);
                                }
                                sub_class.superclass = Some(super_ref);
                            }
                        }
                    }
                }
                OpCode::GetSuper => {
                    let idx = self.read_u16(&chunk);
                    let id = self.get_constant_intern_id(&chunk, idx)?;
                    let superclass = self.fiber.pop();
                    let receiver = self.fiber.pop();
                    if let Value::Object(super_ref) = superclass {
                        let method_ref = {
                            let sc = self
                                .heap
                                .get(super_ref)
                                .ok_or_else(|| NiError::runtime("Invalid superclass"))?;
                            let sc = sc
                                .as_class()
                                .ok_or_else(|| NiError::runtime("Not a class"))?;
                            sc.methods.get(&id).cloned().ok_or_else(|| {
                                NiError::runtime(format!(
                                    "Undefined method '{}'",
                                    self.interner.resolve(id)
                                ))
                            })?
                        };
                        let bound = NiObject::BoundMethod(BoundMethod {
                            receiver,
                            method: method_ref,
                        });
                        let r = self.heap.alloc(bound);
                        self.fiber.push(Value::Object(r));
                    }
                }
                OpCode::Invoke => {
                    let idx = self.read_u16(&chunk);
                    let id = self.get_constant_intern_id(&chunk, idx)?;
                    let arg_count = self.read_byte(&chunk) as usize;
                    self.invoke(id, arg_count)?;
                }
                OpCode::SuperInvoke => {
                    let idx = self.read_u16(&chunk);
                    let id = self.get_constant_intern_id(&chunk, idx)?;
                    let arg_count = self.read_byte(&chunk) as usize;
                    // Get self (receiver) to find its class's superclass
                    let receiver_idx = self.fiber.stack.len() - 1 - arg_count;
                    let receiver = self.fiber.stack[receiver_idx].clone();
                    if let Value::Object(receiver_ref) = receiver {
                        // Find the superclass through the instance's class
                        let super_ref = {
                            let obj = self
                                .heap
                                .get(receiver_ref)
                                .ok_or_else(|| NiError::runtime("Invalid receiver"))?;
                            let class_ref = match obj {
                                NiObject::Instance(inst) => inst.class,
                                _ => return Err(NiError::runtime("super used outside of class")),
                            };
                            let class = self
                                .heap
                                .get(class_ref)
                                .ok_or_else(|| NiError::runtime("Invalid class"))?;
                            let class = class
                                .as_class()
                                .ok_or_else(|| NiError::runtime("Not a class"))?;
                            class
                                .superclass
                                .ok_or_else(|| NiError::runtime("No superclass"))?
                        };
                        let method_ref = {
                            let sc = self
                                .heap
                                .get(super_ref)
                                .ok_or_else(|| NiError::runtime("Invalid superclass"))?;
                            let sc = sc
                                .as_class()
                                .ok_or_else(|| NiError::runtime("Not a class"))?;
                            sc.methods.get(&id).cloned().ok_or_else(|| {
                                NiError::runtime(format!(
                                    "Undefined method '{}'",
                                    self.interner.resolve(id)
                                ))
                            })?
                        };
                        self.call_closure(method_ref, arg_count)?;
                    }
                }

                OpCode::BuildList => {
                    let count = self.read_u16(&chunk) as usize;
                    let count = count.min(self.fiber.stack.len());
                    let start = self.fiber.stack.len() - count;
                    let items: Vec<Value> = self.fiber.stack.drain(start..).collect();
                    let r = self.heap.alloc(NiObject::List(items));
                    self.fiber.push(Value::Object(r));
                }
                OpCode::BuildMap => {
                    let count = self.read_u16(&chunk) as usize;
                    let pair_count = (count * 2).min(self.fiber.stack.len());
                    let start = self.fiber.stack.len() - pair_count;
                    let pairs: Vec<Value> = self.fiber.stack.drain(start..).collect();
                    let entries: Vec<(Value, Value)> = pairs
                        .chunks(2)
                        .map(|c| (c[0].clone(), c[1].clone()))
                        .collect();
                    let r = self.heap.alloc(NiObject::Map(entries));
                    self.fiber.push(Value::Object(r));
                }
                OpCode::BuildRange => {
                    let inclusive = self.read_byte(&chunk) != 0;
                    let end = self.fiber.pop();
                    let start = self.fiber.pop();
                    let (s, e) = match (start, end) {
                        (Value::Int(s), Value::Int(e)) => (s, e),
                        _ => return Err(NiError::runtime("Range bounds must be integers")),
                    };
                    let r = self.heap.alloc(NiObject::Range(NiRange {
                        start: s,
                        end: e,
                        inclusive,
                        step: 1,
                    }));
                    self.fiber.push(Value::Object(r));
                }

                OpCode::GetIterator => {
                    let val = self.fiber.pop();
                    let iter = self.make_iterator(val)?;
                    let r = self.heap.alloc(NiObject::Iterator(iter));
                    self.fiber.push(Value::Object(r));
                }
                OpCode::IteratorNext => {
                    let jump_offset = self.read_u16(&chunk) as usize;
                    let iter_val = self.fiber.pop(); // pop the iterator copy pushed by GetLocal
                    if let Value::Object(iter_ref) = iter_val {
                        match self.iterator_next(iter_ref)? {
                            Some(val) => {
                                self.fiber.push(val);
                            }
                            std::option::Option::None => {
                                self.fiber.frames.last_mut().unwrap().ip += jump_offset;
                            }
                        }
                    }
                }

                OpCode::SpawnFiber => {
                    // Pop the closure and create a real fiber in the ready queue
                    let closure = self.fiber.pop();
                    if let Value::Object(closure_ref) = closure {
                        let new_fiber = Fiber::new(closure_ref);
                        let fid = FiberId::next();
                        self.ready_fibers.push_back((fid, new_fiber));
                        // Push the fiber ID as an int on the current stack
                        self.fiber.push(Value::Int(fid.0 as i64));
                    } else {
                        return Err(NiError::runtime("spawn requires a function"));
                    }
                }
                OpCode::Yield => {
                    // Pop the yielded value and store it as the fiber's result
                    let yielded = self.fiber.pop();
                    self.fiber.result = Some(yielded);
                    self.fiber.state = FiberState::Suspended;
                    return Ok(RunResult::Suspended);
                }
                OpCode::Wait => {
                    // Pop the duration (seconds) and set the fiber's wait timer
                    let duration = self.fiber.pop();
                    let seconds = match duration {
                        Value::Float(f) => f,
                        Value::Int(i) => i as f64,
                        _ => return Err(NiError::runtime("wait requires a number (seconds)")),
                    };
                    if seconds < 0.0 {
                        return Err(NiError::runtime("wait duration must be non-negative"));
                    }
                    self.fiber.wait_timer = seconds;
                    self.fiber.state = FiberState::Suspended;
                    // Push None as the result of the wait expression
                    self.fiber.push(Value::None);
                    return Ok(RunResult::Suspended);
                }
                OpCode::Await => {
                    if let Some((token, call_name, call_args)) = self.last_pending.take() {
                        // The native returned Pending -- park this fiber.
                        let call = SubstrateCall {
                            name: call_name,
                            args: call_args,
                            fiber_id: self.current_fiber_id,
                            component: String::new(),
                        };
                        return Ok(RunResult::Parked(token, call));
                    }
                    // Otherwise the call returned Ready -- value is already on stack, no-op.
                }

                OpCode::StringConcat => {
                    let count = self.read_u16(&chunk) as usize;
                    let start = self.fiber.stack.len() - count;
                    let parts: Vec<String> = self.fiber.stack[start..]
                        .iter()
                        .map(|v| native::value_to_display_string(v, &self.heap, &self.interner))
                        .collect();
                    self.fiber.stack.truncate(start);
                    let result = parts.join("");
                    let r = self.heap.alloc(NiObject::String(result));
                    self.fiber.push(Value::Object(r));
                }

                OpCode::SafeNav => {
                    let idx = self.read_u16(&chunk);
                    let id = self.get_constant_intern_id(&chunk, idx)?;
                    let receiver = self.fiber.pop();
                    if receiver.is_none() {
                        self.fiber.push(Value::None);
                    } else {
                        let result = self.get_property(receiver, id)?;
                        self.fiber.push(result);
                    }
                }
                OpCode::NoneCoalesce => {
                    let jump_offset = self.read_u16(&chunk) as usize;
                    let val = self.fiber.peek(0);
                    if !val.is_none() {
                        self.fiber.frames.last_mut().unwrap().ip += jump_offset;
                    } else {
                        self.fiber.pop();
                    }
                }

                OpCode::Fail => {
                    let fail_value = self.fiber.pop();

                    // Check catch points first (try-expression)
                    if let Some(cp) = self.fiber.catch_points.pop() {
                        // Unwind frames to the catch point's frame
                        while self.fiber.frames.len() > cp.frame_idx + 1 {
                            let frame = self.fiber.frames.pop().unwrap();
                            self.close_upvalues(frame.stack_base);
                        }
                        // Truncate stack to saved size
                        self.fiber.stack.truncate(cp.stack_size);
                        // Push None as result (try-expr swallows the error)
                        self.fiber.push(Value::None);
                        // Jump to handler
                        self.fiber.frames[cp.frame_idx].ip = cp.handler_ip;
                    } else if let Some((frame_idx, handler_ip, stack_depth)) =
                        self.find_exception_handler()
                    {
                        // Unwind frames to the handler's frame
                        while self.fiber.frames.len() > frame_idx + 1 {
                            let frame = self.fiber.frames.pop().unwrap();
                            self.close_upvalues(frame.stack_base);
                        }
                        // Truncate stack to the handler's expected depth
                        let stack_base = self.fiber.frames[frame_idx].stack_base;
                        self.fiber.stack.truncate(stack_base + stack_depth);
                        // Push raw fail value for catch block
                        self.fiber.push(fail_value);
                        // Jump to handler
                        self.fiber.frames[frame_idx].ip = handler_ip;
                    } else {
                        let message = native::value_to_display_string(
                            &fail_value,
                            &self.heap,
                            &self.interner,
                        );
                        match self.fail_policy {
                            FailPolicy::Error => {
                                return Err(NiError::runtime(format!(
                                    "Uncaught error: {}",
                                    message
                                )));
                            }
                            FailPolicy::Log => {
                                self.output.push(format!("[fail] {}", message));
                                self.fiber.push(Value::None);
                            }
                        }
                    }
                }

                OpCode::SetCatchPoint => {
                    let handler_offset = self.read_u16(&chunk) as usize;
                    let handler_ip = self.fiber.frames.last().unwrap().ip + handler_offset;
                    self.fiber.catch_points.push(crate::fiber::CatchPoint {
                        stack_size: self.fiber.stack.len(),
                        frame_idx: self.fiber.frames.len() - 1,
                        handler_ip,
                    });
                }
                OpCode::ClearCatchPoint => {
                    self.fiber.catch_points.pop();
                }

                OpCode::Print => {
                    let val = self.fiber.pop();
                    let s = native::value_to_display_string(&val, &self.heap, &self.interner);
                    self.output.push(s.clone());
                    if !self.suppress_print {
                        println!("{}", s);
                    }
                }

                OpCode::AssertOp => {
                    let has_message = self.read_byte(&chunk);
                    let message = if has_message == 1 {
                        let msg = self.fiber.pop();
                        Some(native::value_to_display_string(
                            &msg,
                            &self.heap,
                            &self.interner,
                        ))
                    } else {
                        None
                    };
                    let condition = self.fiber.pop();
                    if self.is_falsy(&condition) {
                        let msg = message.unwrap_or_else(|| "Assertion failed".to_string());
                        return Err(NiError {
                            message: msg,
                            span: Some(ni_error::Span::new(0, 0, current_line, current_line, 0, 0)),
                            kind: ni_error::ErrorKind::Runtime,
                        });
                    }
                }

                OpCode::AssertCmp => {
                    let cmp_op = self.read_byte(&chunk);
                    let right = self.fiber.pop();
                    let left = self.fiber.pop();
                    let passed = match cmp_op {
                        0 => self.values_equal(&left, &right),  // ==
                        1 => !self.values_equal(&left, &right), // !=
                        2 => {
                            self.compare_values(&left, &right, |o| o == std::cmp::Ordering::Less)?
                        }
                        3 => self
                            .compare_values(&left, &right, |o| o == std::cmp::Ordering::Greater)?,
                        4 => self
                            .compare_values(&left, &right, |o| o != std::cmp::Ordering::Greater)?,
                        5 => {
                            self.compare_values(&left, &right, |o| o != std::cmp::Ordering::Less)?
                        }
                        _ => false,
                    };
                    if !passed {
                        let left_str =
                            native::value_to_display_string(&left, &self.heap, &self.interner);
                        let right_str =
                            native::value_to_display_string(&right, &self.heap, &self.interner);
                        let op_str = match cmp_op {
                            0 => "==",
                            1 => "!=",
                            2 => "<",
                            3 => ">",
                            4 => "<=",
                            5 => ">=",
                            _ => "??",
                        };
                        let msg = format!(
                            "Assertion failed: {} {} {}\n  expected: {}\n   but was: {}",
                            left_str, op_str, right_str, right_str, left_str
                        );
                        return Err(NiError {
                            message: msg,
                            span: Some(ni_error::Span::new(0, 0, current_line, current_line, 0, 0)),
                            kind: ni_error::ErrorKind::Runtime,
                        });
                    }
                }

                OpCode::SetDocstring => {
                    let ds_val = self.fiber.pop();
                    let ds = if let Value::Object(r) = &ds_val {
                        match self.heap.get(*r) {
                            Some(NiObject::String(s)) => s.clone(),
                            Some(NiObject::InternedString(id)) => {
                                self.interner.resolve(*id).to_string()
                            }
                            _ => String::new(),
                        }
                    } else {
                        String::new()
                    };
                    let class_val = self.fiber.peek(0).clone();
                    if let Value::Object(class_ref) = class_val {
                        if let Some(class) = self.heap.get_mut(class_ref) {
                            if let Some(c) = class.as_class_mut() {
                                c.docstring = Some(ds);
                            }
                        }
                    }
                }
            }

            // Check memory limit
            if let Some(limit) = self.memory_limit {
                if self.heap.bytes_allocated() > limit {
                    return Err(NiError::runtime(format!(
                        "Memory limit exceeded ({} bytes, limit {} bytes)",
                        self.heap.bytes_allocated(),
                        limit
                    )));
                }
            }

            // Trigger GC if needed
            if self.gc_enabled && self.heap.should_collect() {
                self.collect_garbage();
            }
        }
    }

    fn read_byte(&mut self, chunk: &crate::chunk::Chunk) -> u8 {
        let frame = self.fiber.frames.last_mut().unwrap();
        let byte = chunk.code[frame.ip];
        frame.ip += 1;
        byte
    }

    fn read_u16(&mut self, chunk: &crate::chunk::Chunk) -> u16 {
        let frame = self.fiber.frames.last_mut().unwrap();
        let val = chunk.read_u16(frame.ip);
        frame.ip += 2;
        val
    }

    fn op_add(&mut self, a: Value, b: Value) -> Result<Value, NiError> {
        match (&a, &b) {
            (Value::Int(a), Value::Int(b)) => a.checked_add(*b).map(Value::Int).ok_or_else(|| NiError::runtime("Integer overflow")),
            (Value::Float(a), Value::Float(b)) => Ok(Value::Float(a + b)),
            (Value::Int(a), Value::Float(b)) => Ok(Value::Float(*a as f64 + b)),
            (Value::Float(a), Value::Int(b)) => Ok(Value::Float(a + *b as f64)),
            (Value::Object(a_ref), Value::Object(b_ref)) => {
                // Try string concatenation first
                let a_str = self.resolve_string_content(*a_ref).map(|s| s.to_string());
                let b_str = self.resolve_string_content(*b_ref).map(|s| s.to_string());
                if let (Some(a), Some(b)) = (a_str, b_str) {
                    let result = a + &b;
                    let r = self.heap.alloc(NiObject::String(result));
                    return Ok(Value::Object(r));
                }
                // Try list concatenation
                let a_list = self.heap.get(*a_ref).and_then(|o| o.as_list()).cloned();
                let b_list = self.heap.get(*b_ref).and_then(|o| o.as_list()).cloned();
                if let (Some(a), Some(b)) = (a_list, b_list) {
                    let mut result = a;
                    result.extend(b);
                    let r = self.heap.alloc(NiObject::List(result));
                    return Ok(Value::Object(r));
                }
                // Try bytes concatenation
                let a_bytes = self.heap.get(*a_ref).and_then(|o| o.as_bytes()).cloned();
                let b_bytes = self.heap.get(*b_ref).and_then(|o| o.as_bytes()).cloned();
                if let (Some(a), Some(b)) = (a_bytes, b_bytes) {
                    let mut result = a;
                    result.extend(b);
                    let r = self.heap.alloc(NiObject::Bytes(result));
                    return Ok(Value::Object(r));
                }
                Err(NiError::runtime("Cannot add these types"))
            }
            _ => Err(NiError::runtime(format!(
                "Cannot add {} and {}",
                a.type_name(),
                b.type_name()
            ))),
        }
    }

    fn op_multiply(&mut self, a: Value, b: Value) -> Result<Value, NiError> {
        match (&a, &b) {
            (Value::Int(a), Value::Int(b)) => a.checked_mul(*b).map(Value::Int).ok_or_else(|| NiError::runtime("Integer overflow")),
            (Value::Float(a), Value::Float(b)) => Ok(Value::Float(a * b)),
            (Value::Int(a), Value::Float(b)) => Ok(Value::Float(*a as f64 * b)),
            (Value::Float(a), Value::Int(b)) => Ok(Value::Float(a * *b as f64)),
            // String repetition: "ha" * 3 = "hahaha"
            (Value::Object(s_ref), Value::Int(n)) => {
                if let Some(s) = self.resolve_string_content(*s_ref) {
                    if *n <= 0 {
                        let r = self.heap.alloc(NiObject::String(String::new()));
                        return Ok(Value::Object(r));
                    }
                    let total = s.len().saturating_mul(*n as usize);
                    if total > MAX_ALLOC_SIZE {
                        return Err(NiError::runtime(format!(
                            "String repetition too large ({} bytes)", total
                        )));
                    }
                    let result = s.repeat(*n as usize);
                    let r = self.heap.alloc(NiObject::String(result));
                    return Ok(Value::Object(r));
                }
                Err(NiError::runtime("Cannot multiply these types"))
            }
            _ => Err(NiError::runtime("Cannot multiply these types")),
        }
    }

    fn values_equal(&self, a: &Value, b: &Value) -> bool {
        match (a, b) {
            (Value::Object(a_ref), Value::Object(b_ref)) => {
                if a_ref == b_ref {
                    return true;
                }
                let a_obj = self.heap.get(*a_ref);
                let b_obj = self.heap.get(*b_ref);
                match (a_obj, b_obj) {
                    (Some(a_o), Some(b_o)) => {
                        let a_str = a_o.as_string_with_intern(&self.interner);
                        let b_str = b_o.as_string_with_intern(&self.interner);
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

    fn compare_values(
        &self,
        a: &Value,
        b: &Value,
        pred: fn(std::cmp::Ordering) -> bool,
    ) -> Result<bool, NiError> {
        let ord = match (a, b) {
            (Value::Int(a), Value::Int(b)) => a.cmp(b),
            (Value::Float(a), Value::Float(b)) => {
                a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal)
            }
            (Value::Int(a), Value::Float(b)) => (*a as f64)
                .partial_cmp(b)
                .unwrap_or(std::cmp::Ordering::Equal),
            (Value::Float(a), Value::Int(b)) => a
                .partial_cmp(&(*b as f64))
                .unwrap_or(std::cmp::Ordering::Equal),
            (Value::Object(a_ref), Value::Object(b_ref)) => {
                let a_str = self.resolve_string_content(*a_ref);
                let b_str = self.resolve_string_content(*b_ref);
                match (a_str, b_str) {
                    (Some(a), Some(b)) => a.cmp(b),
                    _ => return Err(NiError::runtime("Cannot compare these types")),
                }
            }
            _ => {
                return Err(NiError::runtime(format!(
                    "Cannot compare {} and {}",
                    a.type_name(),
                    b.type_name()
                )))
            }
        };
        Ok(pred(ord))
    }

    fn is_falsy(&self, val: &Value) -> bool {
        match val {
            Value::Bool(b) => !b,
            Value::None => true,
            Value::Int(0) => true,
            Value::Float(f) => *f == 0.0,
            Value::Object(r) => match self.heap.get(*r) {
                Some(NiObject::InternedString(id)) => self.interner.resolve(*id).is_empty(),
                Some(o) => o.is_falsy(),
                None => true,
            },
            _ => false,
        }
    }

    fn call_value(&mut self, arg_count: usize) -> Result<(), NiError> {
        let callee_idx = self.fiber.stack.len() - 1 - arg_count;
        let callee = self.fiber.stack[callee_idx].clone();

        match callee {
            Value::Object(r) => {
                let obj = self
                    .heap
                    .get(r)
                    .ok_or_else(|| NiError::runtime("Invalid callee"))?
                    .clone();
                match obj {
                    NiObject::Closure(_) => {
                        self.call_closure(r, arg_count)?;
                    }
                    NiObject::NativeFunction(native_fn) => {
                        let start = self.fiber.stack.len() - arg_count;
                        let args: Vec<Value> = self.fiber.stack[start..].to_vec();
                        let is_print = native_fn.name == "print";
                        let fn_name = native_fn.name.clone();

                        // Debug: on_substrate_call
                        if let Some(ref mut obs) = self.observer {
                            let action = obs.on_substrate_call(&fn_name, &args);
                            self.handle_debug_action(action, self.last_debug_line)?;
                        }

                        self.fiber.stack.truncate(callee_idx);
                        if is_print {
                            // Handle print specially to capture output
                            let parts: Vec<String> = args
                                .iter()
                                .map(|v| {
                                    native::value_to_display_string(v, &self.heap, &self.interner)
                                })
                                .collect();
                            let s = parts.join(" ");
                            self.output.push(s.clone());
                            if !self.suppress_print {
                                println!("{}", s);
                            }
                            self.fiber.push(Value::None);

                            // Debug: on_substrate_return
                            if let Some(ref mut obs) = self.observer {
                                obs.on_substrate_return(&fn_name, &Value::None);
                            }
                        } else {
                            match (native_fn.function)(&args, &mut self.heap, &self.interner) {
                                NativeResult::Ready(result) => {
                                    self.fiber.push(result.clone());
                                    if let Some(ref mut obs) = self.observer {
                                        obs.on_substrate_return(&fn_name, &result);
                                    }
                                }
                                NativeResult::Pending(token) => {
                                    // Store pending info; push None placeholder
                                    self.last_pending =
                                        Some((token, fn_name.clone(), args.clone()));
                                    self.fiber.push(Value::None);
                                }
                                NativeResult::Error(e) => {
                                    return self.handle_runtime_error(e);
                                }
                            }
                        }
                    }
                    NiObject::Class(class) => {
                        // Create instance
                        let instance = NiObject::Instance(NiInstance {
                            class: r,
                            fields: class.fields.clone(),
                        });
                        let instance_ref = self.heap.alloc(instance);
                        self.fiber.stack[callee_idx] = Value::Object(instance_ref);

                        // Call init if it exists
                        if let Some(init_ref) = class.methods.get(&self.init_id).cloned() {
                            self.call_closure(init_ref, arg_count)?;
                        } else if arg_count != 0 {
                            return Err(NiError::runtime(format!(
                                "{} takes no arguments ({} given)",
                                class.name, arg_count
                            )));
                        }
                    }
                    NiObject::NativeClass(nc) => {
                        // Create instance backed by a NativeClass
                        let instance = NiObject::Instance(NiInstance {
                            class: r,
                            fields: HashMap::new(),
                        });
                        let instance_ref = self.heap.alloc(instance);
                        self.fiber.stack[callee_idx] = Value::Object(instance_ref);

                        // Call native init if it exists
                        if let Some(init_fn) = nc.methods.get(&self.init_id).cloned() {
                            let start = self.fiber.stack.len() - arg_count;
                            let args: Vec<Value> = self.fiber.stack[start..].to_vec();
                            // args[0..] are the constructor args; prepend the instance as receiver
                            let mut full_args = vec![Value::Object(instance_ref)];
                            full_args.extend(args);
                            self.fiber.stack.truncate(callee_idx);
                            match (init_fn.function)(&full_args, &mut self.heap, &self.interner) {
                                NativeResult::Ready(_) => {
                                    self.fiber.push(Value::Object(instance_ref));
                                }
                                NativeResult::Pending(_) => {
                                    return Err(NiError::runtime(
                                        "Async native called without await -- use run_ready()",
                                    ));
                                }
                                NativeResult::Error(e) => {
                                    return self.handle_runtime_error(e);
                                }
                            }
                        } else if arg_count != 0 {
                            return Err(NiError::runtime(format!(
                                "{} takes no arguments ({} given)",
                                nc.name, arg_count
                            )));
                        } else {
                            // No init, no args -- just clean up stack and push instance
                            self.fiber.stack.truncate(callee_idx);
                            self.fiber.push(Value::Object(instance_ref));
                        }
                    }
                    NiObject::BoundMethod(bm) => {
                        self.fiber.stack[callee_idx] = bm.receiver.clone();
                        self.call_closure(bm.method, arg_count)?;
                    }
                    _ => return Err(NiError::runtime("Not a callable value")),
                }
            }
            _ => return Err(NiError::runtime("Not a callable value")),
        }
        Ok(())
    }

    fn call_closure(&mut self, closure_ref: GcRef, arg_count: usize) -> Result<(), NiError> {
        if self.fiber.frames.len() >= MAX_FRAMES {
            return Err(NiError::runtime("Stack overflow"));
        }

        let (arity, default_count) = {
            let closure = self
                .heap
                .get(closure_ref)
                .ok_or_else(|| NiError::runtime("Invalid closure"))?;
            let closure = closure
                .as_closure()
                .ok_or_else(|| NiError::runtime("Expected closure"))?;
            let func = self
                .heap
                .get(closure.function)
                .ok_or_else(|| NiError::runtime("Invalid function"))?;
            let func = func
                .as_function()
                .ok_or_else(|| NiError::runtime("Expected function"))?;
            (func.arity as usize, func.default_count as usize)
        };

        let min_arity = arity - default_count;
        if arg_count < min_arity || arg_count > arity {
            return Err(NiError::runtime(format!(
                "Expected {} to {} arguments but got {}",
                min_arity, arity, arg_count
            )));
        }

        // Pad with None for default params that weren't provided
        for _ in arg_count..arity {
            self.fiber.push(Value::None);
        }

        let stack_base = self.fiber.stack.len() - arity - 1;
        self.fiber.frames.push(CallFrame {
            closure: closure_ref,
            ip: 0,
            stack_base,
        });

        Ok(())
    }

    fn invoke(&mut self, id: InternId, arg_count: usize) -> Result<(), NiError> {
        let receiver_idx = self.fiber.stack.len() - 1 - arg_count;
        let receiver = self.fiber.stack[receiver_idx].clone();
        let name = self.interner.resolve(id).to_string();

        if let Value::Object(receiver_ref) = &receiver {
            // Try stdlib methods first
            let args: Vec<Value> = self.fiber.stack[receiver_idx + 1..].to_vec();
            match stdlib::call_method(&mut self.heap, *receiver_ref, &name, &args, &self.interner) {
                Ok(Some(result)) => {
                    self.fiber.stack.truncate(receiver_idx);
                    self.fiber.push(result);
                    return Ok(());
                }
                Err(e) => {
                    self.fiber.stack.truncate(receiver_idx);
                    return self.handle_runtime_error(e);
                }
                Ok(None) => {} // Not a stdlib method, fall through
            }

            // Try instance method
            let obj = self.heap.get(*receiver_ref).cloned();
            if let Some(NiObject::Instance(instance)) = obj {
                // Check for field that might be a closure
                if let Some(field_val) = instance.fields.get(&id).cloned() {
                    self.fiber.stack[receiver_idx] = field_val;
                    return self.call_value(arg_count);
                }
                // Look up method on class (NiClass)
                let class = self.heap.get(instance.class).cloned();
                if let Some(NiObject::Class(class)) = class {
                    if let Some(method_ref) = class.methods.get(&id).cloned() {
                        return self.call_closure(method_ref, arg_count);
                    }
                }
                // Look up method on native class
                if let Some(NiObject::NativeClass(nc)) = self.heap.get(instance.class).cloned() {
                    if let Some(native_fn) = nc.methods.get(&id).cloned() {
                        let args: Vec<Value> = self.fiber.stack[receiver_idx..].to_vec();
                        self.fiber.stack.truncate(receiver_idx);
                        match (native_fn.function)(&args, &mut self.heap, &self.interner) {
                            NativeResult::Ready(result) => {
                                self.fiber.push(result);
                                return Ok(());
                            }
                            NativeResult::Pending(_) => {
                                return Err(NiError::runtime(
                                    "Async native called without await -- use run_ready()",
                                ));
                            }
                            NativeResult::Error(e) => {
                                return self.handle_runtime_error(e);
                            }
                        }
                    }
                }
                return Err(NiError::runtime(format!("Undefined method '{}'", name)));
            }

            // Try class static method
            if let Some(NiObject::Class(class)) = self.heap.get(*receiver_ref).cloned() {
                if let Some(method_ref) = class.methods.get(&id).cloned() {
                    return self.call_closure(method_ref, arg_count);
                }
            }

            // Try native class static method
            if let Some(NiObject::NativeClass(nc)) = self.heap.get(*receiver_ref).cloned() {
                if let Some(native_fn) = nc.static_methods.get(&id).cloned() {
                    let args: Vec<Value> = self.fiber.stack[receiver_idx + 1..].to_vec();
                    self.fiber.stack.truncate(receiver_idx);
                    match (native_fn.function)(&args, &mut self.heap, &self.interner) {
                        NativeResult::Ready(result) => {
                            self.fiber.push(result);
                            return Ok(());
                        }
                        NativeResult::Pending(_) => {
                            return Err(NiError::runtime(
                                "Async native called without await -- use run_ready()",
                            ));
                        }
                        NativeResult::Error(e) => {
                            return self.handle_runtime_error(e);
                        }
                    }
                }
            }

            // Try enum variant
            if let Some(NiObject::Enum(e)) = self.heap.get(*receiver_ref) {
                if let Some(val) = e.variants.get(&id) {
                    let val = val.clone();
                    self.fiber.stack.truncate(receiver_idx);
                    self.fiber.push(val);
                    return Ok(());
                }
            }

            // Try map field that might be callable
            if let Some(NiObject::Map(entries)) = self.heap.get(*receiver_ref).cloned() {
                for (k, v) in &entries {
                    if let Value::Object(kr) = k {
                        if let Some(s) = self.resolve_string_content(*kr) {
                            if s == name {
                                self.fiber.stack[receiver_idx] = v.clone();
                                return self.call_value(arg_count);
                            }
                        }
                    }
                }
            }
        }

        Err(NiError::runtime(format!(
            "Cannot call method '{}' on {}",
            name,
            receiver.type_name()
        )))
    }

    fn get_property(&mut self, receiver: Value, id: InternId) -> Result<Value, NiError> {
        let name = self.interner.resolve(id).to_string();
        if let Value::Object(receiver_ref) = &receiver {
            // Try stdlib property first
            if let Some(val) =
                stdlib::get_property(&mut self.heap, *receiver_ref, &name, &self.interner)
                    .map_err(NiError::runtime)?
            {
                return Ok(val);
            }

            let obj = self.heap.get(*receiver_ref).cloned();
            match obj {
                Some(NiObject::Instance(instance)) => {
                    if let Some(val) = instance.fields.get(&id) {
                        return Ok(val.clone());
                    }
                    // Look for method on NiClass
                    let class = self.heap.get(instance.class).cloned();
                    if let Some(NiObject::Class(class)) = class {
                        if let Some(method_ref) = class.methods.get(&id).cloned() {
                            let bound = NiObject::BoundMethod(BoundMethod {
                                receiver: receiver.clone(),
                                method: method_ref,
                            });
                            let r = self.heap.alloc(bound);
                            return Ok(Value::Object(r));
                        }
                    }
                    // Look for method on NativeClass -- wrap as a NativeFunction bound method
                    if let Some(NiObject::NativeClass(nc)) = self.heap.get(instance.class).cloned()
                    {
                        if let Some(native_fn) = nc.methods.get(&id).cloned() {
                            let nf_ref = self.heap.alloc(NiObject::NativeFunction(native_fn));
                            let bound = NiObject::BoundMethod(BoundMethod {
                                receiver: receiver.clone(),
                                method: nf_ref,
                            });
                            let r = self.heap.alloc(bound);
                            return Ok(Value::Object(r));
                        }
                    }
                    Err(NiError::runtime(format!("Undefined property '{}'", name)))
                }
                Some(NiObject::Closure(closure)) => {
                    if name == "doc" {
                        if let Some(func) = self.heap.get(closure.function) {
                            if let Some(f) = func.as_function() {
                                if let Some(ref ds) = f.docstring {
                                    let r = self.heap.alloc(NiObject::String(ds.clone()));
                                    return Ok(Value::Object(r));
                                }
                            }
                        }
                        return Ok(Value::None);
                    }
                    Err(NiError::runtime(format!(
                        "Cannot access property '{}' on closure",
                        name
                    )))
                }
                Some(NiObject::Class(class)) => {
                    if name == "doc" {
                        if let Some(ref ds) = class.docstring {
                            let r = self.heap.alloc(NiObject::String(ds.clone()));
                            return Ok(Value::Object(r));
                        }
                        return Ok(Value::None);
                    }
                    // Static field or method
                    if let Some(val) = class.fields.get(&id) {
                        return Ok(val.clone());
                    }
                    if let Some(method_ref) = class.methods.get(&id).cloned() {
                        return Ok(Value::Object(method_ref));
                    }
                    Err(NiError::runtime(format!(
                        "Undefined static member '{}'",
                        name
                    )))
                }
                Some(NiObject::Enum(e)) => {
                    if let Some(val) = e.variants.get(&id) {
                        return Ok(val.clone());
                    }
                    Err(NiError::runtime(format!("Unknown enum variant '{}'", name)))
                }
                Some(NiObject::Map(entries)) => {
                    for (k, v) in entries {
                        if let Value::Object(kr) = k {
                            if let Some(s) = self.resolve_string_content(kr) {
                                if s == name {
                                    return Ok(v.clone());
                                }
                            }
                        }
                    }
                    Err(NiError::runtime(format!("Undefined property '{}'", name)))
                }
                _ => Err(NiError::runtime(format!(
                    "Cannot access property '{}' on {}",
                    name,
                    receiver.type_name()
                ))),
            }
        } else {
            Err(NiError::runtime(format!(
                "Cannot access property '{}' on {}",
                name,
                receiver.type_name()
            )))
        }
    }

    fn set_property(&mut self, receiver: Value, id: InternId, value: Value) -> Result<(), NiError> {
        if let Value::Object(receiver_ref) = receiver {
            let obj = self
                .heap
                .get_mut(receiver_ref)
                .ok_or_else(|| NiError::runtime("Invalid object"))?;
            match obj {
                NiObject::Instance(instance) => {
                    instance.fields.insert(id, value);
                    Ok(())
                }
                NiObject::Class(class) => {
                    class.fields.insert(id, value);
                    Ok(())
                }
                _ => Err(NiError::runtime("Cannot set property on this value")),
            }
        } else {
            Err(NiError::runtime("Cannot set property on non-object"))
        }
    }

    fn get_index(&mut self, receiver: Value, index: Value) -> Result<Value, NiError> {
        if let Value::Object(r) = &receiver {
            let obj = self
                .heap
                .get(*r)
                .ok_or_else(|| NiError::runtime("Invalid object"))?;
            match obj {
                NiObject::List(list) => {
                    let idx = index
                        .as_int()
                        .ok_or_else(|| NiError::runtime("List index must be an integer"))?;
                    let len = list.len() as i64;
                    let actual_idx = if idx < 0 { len + idx } else { idx };
                    if actual_idx < 0 || actual_idx >= len {
                        return Err(NiError::runtime(format!(
                            "Index {} out of bounds (length {})",
                            idx, len
                        )));
                    }
                    Ok(list[actual_idx as usize].clone())
                }
                NiObject::Bytes(bytes) => {
                    let idx = index
                        .as_int()
                        .ok_or_else(|| NiError::runtime("Bytes index must be an integer"))?;
                    let len = bytes.len() as i64;
                    let actual_idx = if idx < 0 { len + idx } else { idx };
                    if actual_idx < 0 || actual_idx >= len {
                        return Err(NiError::runtime(format!(
                            "Index {} out of bounds (length {})",
                            idx, len
                        )));
                    }
                    Ok(Value::Int(bytes[actual_idx as usize] as i64))
                }
                NiObject::Map(map) => {
                    let val = map
                        .iter()
                        .find(|(k, _)| self.values_equal(k, &index))
                        .map(|(_, v)| v.clone())
                        .unwrap_or(Value::None);
                    Ok(val)
                }
                NiObject::String(s) => {
                    let idx = index
                        .as_int()
                        .ok_or_else(|| NiError::runtime("String index must be an integer"))?;
                    let len = s.chars().count() as i64;
                    let actual_idx = if idx < 0 { len + idx } else { idx };
                    if actual_idx < 0 || actual_idx >= len {
                        return Err(NiError::runtime(format!("Index {} out of bounds", idx)));
                    }
                    let ch = s
                        .chars()
                        .nth(actual_idx as usize)
                        .map(|c| c.to_string())
                        .unwrap_or_default();
                    let r = self.heap.alloc(NiObject::String(ch));
                    Ok(Value::Object(r))
                }
                NiObject::InternedString(id) => {
                    let s = self.interner.resolve(*id);
                    let idx = index
                        .as_int()
                        .ok_or_else(|| NiError::runtime("String index must be an integer"))?;
                    let len = s.chars().count() as i64;
                    let actual_idx = if idx < 0 { len + idx } else { idx };
                    if actual_idx < 0 || actual_idx >= len {
                        return Err(NiError::runtime(format!("Index {} out of bounds", idx)));
                    }
                    let ch = s
                        .chars()
                        .nth(actual_idx as usize)
                        .map(|c| c.to_string())
                        .unwrap_or_default();
                    let r = self.heap.alloc(NiObject::String(ch));
                    Ok(Value::Object(r))
                }
                _ => Err(NiError::runtime("Cannot index this type")),
            }
        } else {
            Err(NiError::runtime("Cannot index non-object"))
        }
    }

    fn set_index(&mut self, receiver: Value, index: Value, value: Value) -> Result<(), NiError> {
        if let Value::Object(r) = receiver {
            let obj = self
                .heap
                .get_mut(r)
                .ok_or_else(|| NiError::runtime("Invalid object"))?;
            match obj {
                NiObject::List(list) => {
                    let idx = index
                        .as_int()
                        .ok_or_else(|| NiError::runtime("List index must be an integer"))?;
                    let len = list.len() as i64;
                    let actual_idx = if idx < 0 { len + idx } else { idx };
                    if actual_idx < 0 || actual_idx >= len {
                        return Err(NiError::runtime("Index out of bounds"));
                    }
                    list[actual_idx as usize] = value;
                    Ok(())
                }
                NiObject::Bytes(bytes) => {
                    let idx = index
                        .as_int()
                        .ok_or_else(|| NiError::runtime("Bytes index must be an integer"))?;
                    let byte_val = value
                        .as_int()
                        .ok_or_else(|| NiError::runtime("Byte value must be an integer"))?;
                    if !(0..=255).contains(&byte_val) {
                        return Err(NiError::runtime(format!(
                            "Byte value {} out of range (0-255)",
                            byte_val
                        )));
                    }
                    let len = bytes.len() as i64;
                    let actual_idx = if idx < 0 { len + idx } else { idx };
                    if actual_idx < 0 || actual_idx >= len {
                        return Err(NiError::runtime("Index out of bounds"));
                    }
                    bytes[actual_idx as usize] = byte_val as u8;
                    Ok(())
                }
                NiObject::Map(map) => {
                    // Update or insert using content comparison (same as get_index / values_equal)
                    let pos = map.iter().position(|(k, _)| {
                        match (k, &index) {
                            (Value::Object(a_ref), Value::Object(b_ref)) => {
                                if a_ref == b_ref {
                                    return true;
                                }
                                // Compare string content via the heap/interner
                                // (map is borrowed from heap, so resolve lazily)
                                false
                            }
                            _ => *k == index,
                        }
                    });
                    // For object keys we need a second pass with heap access.
                    // Re-check: if pos not found and index is an Object, try content match.
                    let pos = if pos.is_none() {
                        if let Value::Object(idx_ref) = &index {
                            // Resolve the index key's string content
                            let idx_content = self
                                .heap
                                .get(*idx_ref)
                                .and_then(|o| o.as_string_with_intern(&self.interner))
                                .map(|s| s.to_string());
                            if let Some(idx_str) = idx_content {
                                // Now get map again (immutably) to find position by string content
                                let obj2 = self.heap.get(r).ok_or_else(|| NiError::runtime("Invalid object"))?;
                                if let NiObject::Map(map2) = obj2 {
                                    map2.iter().position(|(k, _)| {
                                        if let Value::Object(k_ref) = k {
                                            self.heap
                                                .get(*k_ref)
                                                .and_then(|o| o.as_string_with_intern(&self.interner))
                                                .map_or(false, |s| s == idx_str)
                                        } else {
                                            false
                                        }
                                    })
                                } else {
                                    None
                                }
                            } else {
                                None
                            }
                        } else {
                            None
                        }
                    } else {
                        pos
                    };
                    let obj = self.heap.get_mut(r).ok_or_else(|| NiError::runtime("Invalid object"))?;
                    if let NiObject::Map(map) = obj {
                        if let Some(p) = pos {
                            map[p].1 = value;
                        } else {
                            map.push((index, value));
                        }
                    }
                    Ok(())
                }
                _ => Err(NiError::runtime("Cannot index-assign this type")),
            }
        } else {
            Err(NiError::runtime("Cannot index-assign non-object"))
        }
    }

    fn make_iterator(&mut self, val: Value) -> Result<NiIterator, NiError> {
        match val {
            Value::Object(r) => {
                let obj = self
                    .heap
                    .get(r)
                    .ok_or_else(|| NiError::runtime("Invalid object"))?;
                match obj {
                    NiObject::Range(range) => Ok(NiIterator::Range {
                        current: range.start,
                        end: range.end,
                        inclusive: range.inclusive,
                        step: range.step,
                    }),
                    NiObject::List(_) => Ok(NiIterator::List { list: r, index: 0 }),
                    NiObject::Bytes(_) => Ok(NiIterator::Bytes { bytes: r, index: 0 }),
                    NiObject::Map(_) => Ok(NiIterator::Map { map: r, index: 0 }),
                    NiObject::String(_) | NiObject::InternedString(_) => Ok(NiIterator::String {
                        string: r,
                        index: 0,
                    }),
                    _ => Err(NiError::runtime("Cannot iterate over this type")),
                }
            }
            _ => Err(NiError::runtime("Cannot iterate over this type")),
        }
    }

    fn iterator_next(&mut self, iter_ref: GcRef) -> Result<Option<Value>, NiError> {
        let iter = self
            .heap
            .get_mut(iter_ref)
            .ok_or_else(|| NiError::runtime("Invalid iterator"))?
            .as_iterator_mut()
            .ok_or_else(|| NiError::runtime("Not an iterator"))?;

        match iter {
            NiIterator::Range {
                current,
                end,
                inclusive,
                step,
            } => {
                let done = if *step > 0 {
                    if *inclusive {
                        *current > *end
                    } else {
                        *current >= *end
                    }
                } else {
                    // negative step: iterate downward
                    if *inclusive {
                        *current < *end
                    } else {
                        *current <= *end
                    }
                };
                if done {
                    Ok(None)
                } else {
                    let val = Value::Int(*current);
                    *current += *step;
                    Ok(Some(val))
                }
            }
            NiIterator::List { list, index } => {
                let list_ref = *list;
                let idx = *index;
                let obj = self.heap.get(list_ref);
                if let Some(NiObject::List(items)) = obj {
                    if idx >= items.len() {
                        Ok(None)
                    } else {
                        let val = items[idx].clone();
                        // Update index
                        let iter = self
                            .heap
                            .get_mut(iter_ref)
                            .unwrap()
                            .as_iterator_mut()
                            .unwrap();
                        if let NiIterator::List { index, .. } = iter {
                            *index = idx + 1;
                        }
                        Ok(Some(val))
                    }
                } else {
                    Ok(None)
                }
            }
            NiIterator::Map { map, index } => {
                let map_ref = *map;
                let idx = *index;
                let obj = self.heap.get(map_ref);
                if let Some(NiObject::Map(entries)) = obj {
                    if idx >= entries.len() {
                        Ok(None)
                    } else {
                        let key = entries[idx].0.clone();
                        let iter = self
                            .heap
                            .get_mut(iter_ref)
                            .unwrap()
                            .as_iterator_mut()
                            .unwrap();
                        if let NiIterator::Map { index, .. } = iter {
                            *index = idx + 1;
                        }
                        Ok(Some(key))
                    }
                } else {
                    Ok(None)
                }
            }
            NiIterator::String { string, index } => {
                let str_ref = *string;
                let idx = *index;
                // Resolve string content (handles both String and InternedString)
                let ch_opt = {
                    let obj = self.heap.get(str_ref);
                    match obj {
                        Some(NiObject::String(s)) => s.chars().nth(idx).map(|c| c.to_string()),
                        Some(NiObject::InternedString(id)) => {
                            let s = self.interner.resolve(*id);
                            s.chars().nth(idx).map(|c| c.to_string())
                        }
                        _ => None,
                    }
                };
                if let Some(ch_str) = ch_opt {
                    let iter = self
                        .heap
                        .get_mut(iter_ref)
                        .unwrap()
                        .as_iterator_mut()
                        .unwrap();
                    if let NiIterator::String { index, .. } = iter {
                        *index = idx + 1;
                    }
                    let r = self.heap.alloc(NiObject::String(ch_str));
                    Ok(Some(Value::Object(r)))
                } else {
                    Ok(None)
                }
            }
            NiIterator::Bytes { bytes, index } => {
                let bytes_ref = *bytes;
                let idx = *index;
                let byte_opt = {
                    let obj = self.heap.get(bytes_ref);
                    if let Some(NiObject::Bytes(b)) = obj {
                        b.get(idx).copied()
                    } else {
                        None
                    }
                };
                if let Some(byte) = byte_opt {
                    let iter = self
                        .heap
                        .get_mut(iter_ref)
                        .unwrap()
                        .as_iterator_mut()
                        .unwrap();
                    if let NiIterator::Bytes { index, .. } = iter {
                        *index = idx + 1;
                    }
                    Ok(Some(Value::Int(byte as i64)))
                } else {
                    Ok(None)
                }
            }
        }
    }

    fn capture_upvalue(&mut self, stack_slot: usize) -> GcRef {
        // Check if we already have an open upvalue for this slot
        for &uv_ref in &self.fiber.open_upvalues {
            if let Some(obj) = self.heap.get(uv_ref) {
                if let Some(UpvalueObj::Open(slot)) = obj.as_upvalue() {
                    if *slot == stack_slot {
                        return uv_ref;
                    }
                }
            }
        }

        let uv = self
            .heap
            .alloc(NiObject::Upvalue(UpvalueObj::Open(stack_slot)));
        self.fiber.open_upvalues.push(uv);
        uv
    }

    fn close_upvalues(&mut self, last: usize) {
        let mut to_close = Vec::new();
        self.fiber.open_upvalues.retain(|&uv_ref| {
            if let Some(obj) = self.heap.get(uv_ref) {
                if let Some(UpvalueObj::Open(slot)) = obj.as_upvalue() {
                    if *slot >= last {
                        to_close.push((uv_ref, *slot));
                        return false;
                    }
                }
            }
            true
        });

        for (uv_ref, slot) in to_close {
            let value = self.fiber.stack.get(slot).cloned().unwrap_or(Value::None);
            if let Some(obj) = self.heap.get_mut(uv_ref) {
                *obj = NiObject::Upvalue(UpvalueObj::Closed(value));
            }
        }
    }

    fn read_upvalue(&self, uv_ref: GcRef) -> Result<Value, NiError> {
        let obj = self
            .heap
            .get(uv_ref)
            .ok_or_else(|| NiError::runtime("Invalid upvalue"))?;
        match obj.as_upvalue() {
            Some(UpvalueObj::Open(slot)) => {
                Ok(self.fiber.stack.get(*slot).cloned().unwrap_or(Value::None))
            }
            Some(UpvalueObj::Closed(val)) => Ok(val.clone()),
            _ => Err(NiError::runtime("Expected upvalue")),
        }
    }

    fn write_upvalue(&mut self, uv_ref: GcRef, value: Value) -> Result<(), NiError> {
        let obj = self
            .heap
            .get(uv_ref)
            .ok_or_else(|| NiError::runtime("Invalid upvalue"))?;
        match obj.as_upvalue() {
            Some(UpvalueObj::Open(slot)) => {
                let slot = *slot;
                self.fiber.stack[slot] = value;
            }
            Some(UpvalueObj::Closed(_)) => {
                if let Some(obj) = self.heap.get_mut(uv_ref) {
                    *obj = NiObject::Upvalue(UpvalueObj::Closed(value));
                }
            }
            _ => return Err(NiError::runtime("Expected upvalue")),
        }
        Ok(())
    }

    fn handle_runtime_error(&mut self, message: String) -> Result<(), NiError> {
        let fail_value = {
            let r = self.heap.alloc(NiObject::String(message.clone()));
            Value::Object(r)
        };

        if let Some(cp) = self.fiber.catch_points.pop() {
            while self.fiber.frames.len() > cp.frame_idx + 1 {
                let frame = self.fiber.frames.pop().unwrap();
                self.close_upvalues(frame.stack_base);
            }
            self.fiber.stack.truncate(cp.stack_size);
            self.fiber.push(Value::None);
            self.fiber.frames[cp.frame_idx].ip = cp.handler_ip;
            Ok(())
        } else if let Some((frame_idx, handler_ip, stack_depth)) = self.find_exception_handler() {
            while self.fiber.frames.len() > frame_idx + 1 {
                let frame = self.fiber.frames.pop().unwrap();
                self.close_upvalues(frame.stack_base);
            }
            let stack_base = self.fiber.frames[frame_idx].stack_base;
            self.fiber.stack.truncate(stack_base + stack_depth);
            self.fiber.push(fail_value);
            self.fiber.frames[frame_idx].ip = handler_ip;
            Ok(())
        } else {
            match self.fail_policy {
                FailPolicy::Error => Err(NiError::runtime(format!("Uncaught error: {}", message))),
                FailPolicy::Log => {
                    self.output.push(format!("[fail] {}", message));
                    self.fiber.push(Value::None);
                    Ok(())
                }
            }
        }
    }

    fn find_exception_handler(&self) -> Option<(usize, usize, usize)> {
        // Walk frames from innermost to outermost
        for frame_idx in (0..self.fiber.frames.len()).rev() {
            let frame = &self.fiber.frames[frame_idx];
            let closure_ref = frame.closure;
            let ip = frame.ip;

            // Get the function's exception table; skip frames with unexpected objects
            let Some(closure) = self.heap.get(closure_ref) else { continue; };
            let Some(closure_data) = closure.as_closure() else { continue; };
            let fn_ref = closure_data.function;
            let Some(func_obj) = self.heap.get(fn_ref) else { continue; };
            let Some(func) = func_obj.as_function() else { continue; };

            for entry in &func.exception_table {
                if ip > entry.try_start && ip <= entry.try_end {
                    return Some((frame_idx, entry.handler_ip, entry.stack_depth));
                }
            }
        }
        None
    }

    fn collect_garbage(&mut self) {
        // Mark roots from main fiber
        for r in self.fiber.gc_roots() {
            self.heap.mark(r);
        }
        // Mark roots from globals
        for val in self.globals.values() {
            if let Value::Object(r) = val {
                self.heap.mark(*r);
            }
        }
        // Mark roots from ready fibers
        for (_, fiber) in &self.ready_fibers {
            for r in fiber.gc_roots() {
                self.heap.mark(r);
            }
        }
        // Mark roots from suspended fibers
        for (_, fiber) in &self.suspended_fibers {
            for r in fiber.gc_roots() {
                self.heap.mark(r);
            }
        }
        // Mark roots from parked fibers
        for (_, fiber) in self.parked_fibers.values() {
            for r in fiber.gc_roots() {
                self.heap.mark(r);
            }
        }
        // Mark host-pinned extra roots
        for r in &self.extra_roots {
            self.heap.mark(*r);
        }

        self.heap.sweep();
    }

    pub fn last_value(&self) -> Value {
        self.fiber.stack.last().cloned().unwrap_or(Value::None)
    }
}

/// Builder for registering native classes with the VM.
pub struct NativeClassBuilder<'a> {
    vm: &'a mut Vm,
    name: String,
    methods: HashMap<InternId, NativeFn>,
    static_methods: HashMap<InternId, NativeFn>,
}

impl<'a> NativeClassBuilder<'a> {
    /// Add an instance method to this native class.
    pub fn method(
        mut self,
        name: &str,
        arity: i8,
        f: fn(&[Value], &mut GcHeap, &InternTable) -> NativeResult,
    ) -> Self {
        let id = self.vm.interner.intern(name);
        self.methods.insert(
            id,
            NativeFn {
                name: name.to_string(),
                arity,
                function: f,
            },
        );
        self
    }

    /// Add a static method to this native class.
    pub fn static_method(
        mut self,
        name: &str,
        arity: i8,
        f: fn(&[Value], &mut GcHeap, &InternTable) -> NativeResult,
    ) -> Self {
        let id = self.vm.interner.intern(name);
        self.static_methods.insert(
            id,
            NativeFn {
                name: name.to_string(),
                arity,
                function: f,
            },
        );
        self
    }

    /// Finalize and register the native class as a global.
    pub fn build(self) {
        let class = NiObject::NativeClass(NativeClass {
            name: self.name.clone(),
            methods: self.methods,
            static_methods: self.static_methods,
        });
        let r = self.vm.heap.alloc(class);
        let id = self.vm.interner.intern(&self.name);
        self.vm.globals.insert(id, Value::Object(r));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;
    use std::rc::Rc;

    #[test]
    fn cancel_parked_fiber() {
        let mut vm = Vm::new();

        // Manually insert a parked fiber
        let token = PendingToken::new();
        let fid = FiberId::next();
        let fiber = Fiber::empty();
        vm.parked_fibers.insert(token, (fid, fiber));
        assert_eq!(vm.parked_count(), 1);

        // Cancel the parked fiber
        assert!(vm.cancel_fiber(fid));
        assert_eq!(vm.parked_count(), 0);

        // Cancelling again should return false
        assert!(!vm.cancel_fiber(fid));
    }

    #[test]
    fn cancel_ready_fiber() {
        let mut vm = Vm::new();

        // Manually insert a ready fiber
        let fid = FiberId::next();
        let fiber = Fiber::empty();
        vm.ready_fibers.push_back((fid, fiber));
        assert_eq!(vm.ready_fibers.len(), 1);

        // Cancel the ready fiber
        assert!(vm.cancel_fiber(fid));
        assert_eq!(vm.ready_fibers.len(), 0);

        // Cancelling again should return false
        assert!(!vm.cancel_fiber(fid));
    }

    #[test]
    fn cancel_fires_observer_for_ready_fiber() {
        let cancelled: Rc<RefCell<Vec<(FiberId, Option<PendingToken>)>>> =
            Rc::new(RefCell::new(Vec::new()));
        let captured = cancelled.clone();

        struct Obs(Rc<RefCell<Vec<(FiberId, Option<PendingToken>)>>>);
        impl VmObserver for Obs {
            fn on_fiber_cancel(&mut self, fid: FiberId, token: Option<PendingToken>) {
                self.0.borrow_mut().push((fid, token));
            }
        }

        let mut vm = Vm::new();
        vm.attach_debugger(Box::new(Obs(captured)));

        let fid = FiberId::next();
        vm.ready_fibers.push_back((fid, Fiber::empty()));

        assert!(vm.cancel_fiber(fid));
        let events = cancelled.borrow();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].0, fid);
        assert!(events[0].1.is_none()); // ready fiber has no token
    }

    #[test]
    fn cancel_fires_observer_and_io_for_parked_fiber() {
        let cancelled: Rc<RefCell<Vec<(FiberId, Option<PendingToken>)>>> =
            Rc::new(RefCell::new(Vec::new()));
        let io_cancelled: Rc<RefCell<Vec<PendingToken>>> = Rc::new(RefCell::new(Vec::new()));
        let obs_cap = cancelled.clone();
        let io_cap = io_cancelled.clone();

        struct Obs(Rc<RefCell<Vec<(FiberId, Option<PendingToken>)>>>);
        impl VmObserver for Obs {
            fn on_fiber_cancel(&mut self, fid: FiberId, token: Option<PendingToken>) {
                self.0.borrow_mut().push((fid, token));
            }
        }

        struct TestIo(Rc<RefCell<Vec<PendingToken>>>);
        impl IoProvider for TestIo {
            fn on_pending(&mut self, _token: PendingToken, _call: SubstrateCall) {}
            fn on_cancel(&mut self, token: PendingToken) {
                self.0.borrow_mut().push(token);
            }
        }

        let mut vm = Vm::new();
        vm.attach_debugger(Box::new(Obs(obs_cap)));
        vm.set_io_provider(Box::new(TestIo(io_cap)));

        let token = PendingToken::new();
        let fid = FiberId::next();
        vm.parked_fibers.insert(token, (fid, Fiber::empty()));

        assert!(vm.cancel_fiber(fid));

        // Observer got the cancel with the token
        let events = cancelled.borrow();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].0, fid);
        assert_eq!(events[0].1, Some(token));

        // IoProvider got the cancel notification
        let io_events = io_cancelled.borrow();
        assert_eq!(io_events.len(), 1);
        assert_eq!(io_events[0], token);
    }

}
