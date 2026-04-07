# Async Execution Model

## Overview

The Ni VM supports cooperative multi-fiber async execution. Native functions can return `Pending` to park a fiber, and the host drives I/O resolution externally -- similar to JavaScript's event loop, but with the host fully in control.

This is the mechanism that lets Knight (device-side runtime) do non-blocking I/O: an HTTP scrape or AT command returns `Pending`, the fiber parks, Knight performs the actual I/O, then calls `resume(token, value)` to deliver the result and wake the fiber.

## Key Types

```rust
// Unique token identifying a pending async operation
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PendingToken(pub u64);

// Unique identifier for a fiber
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct FiberId(pub u64);

// What a native function returns (replaces the old Result<Value, String>)
pub enum NativeResult {
    Ready(Value),              // Synchronous result -- value available now
    Pending(PendingToken),     // Async -- park the fiber, host will resolve later
    Error(String),             // Synchronous error
}

// Scheduler status after run_ready()
pub enum VmStatus {
    AllDone,    // No fibers left (all finished)
    AllParked,  // All remaining fibers are waiting on async I/O
    Suspended,  // Some fibers are suspended (yield/wait) -- call run_ready() again
    Mixed,      // Some fibers ready, some parked
}

// Metadata about a pending native call (for debugger/tracing)
pub struct SubstrateCall {
    pub function_name: String,
    pub args: Vec<Value>,
    pub token: PendingToken,
}

// VM configuration
pub struct VmConfig {
    pub memory_limit: usize,       // bytes, default 32MB
    pub instruction_limit: usize,  // per run_ready() call, default 10M
    pub gc_threshold: usize,       // bytes allocated before GC triggers
    pub enable_specs: bool,        // compile spec blocks (false for production)
}

// Runtime statistics
pub struct VmStats {
    pub total_instructions: u64,
    pub gc_collections: u64,
    pub heap_size: usize,
    pub fiber_count: usize,        // ready + parked
}
```

## Execution Flow

### Simple (No Async)

The traditional path still works exactly as before:

```rust
let mut vm = Vm::new();
vm.run(source)?;  // compiles and runs to completion
```

### Async Host Loop

For hosts that need non-blocking I/O:

```rust
let mut vm = Vm::with_config(VmConfig { .. });

// 1. Register async-capable native functions
vm.register_native("http_get", 1, |args, heap, intern| -> NativeResult {
    let token = PendingToken::new();
    // Host records the token + URL for later resolution
    NativeResult::Pending(token)
});

// 2. Load the script (compile + set up main fiber)
let closure = compile(&program, &mut vm.heap, &mut vm.interner)?;
vm.load(closure)?;

// 3. Run until all fibers park or finish
// delta_time is elapsed seconds since last call (for wait timers)
let mut last_time = std::time::Instant::now();
loop {
    let now = std::time::Instant::now();
    let dt = now.duration_since(last_time).as_secs_f64();
    last_time = now;

    let status = vm.run_ready(dt)?;
    match status {
        VmStatus::AllDone => break,
        VmStatus::AllParked => {
            // All fibers waiting on I/O -- ask the IoProvider or poll yourself
            // When I/O completes, resume the parked fiber:
            vm.resume(token, result_value)?;
        }
        VmStatus::Suspended => continue, // yield/wait fibers -- tick again
        VmStatus::Mixed => continue,     // more ready fibers to drain
    }
}
```

### What Happens Inside

1. A native function returns `NativeResult::Pending(token)` during execution
2. The VM stores the token in `last_pending` and pushes `None` as a placeholder
3. When the `await` opcode executes, it checks `last_pending`:
   - If set: the fiber is **parked** with state `FiberState::Parked`, moved to `parked_fibers`
   - If not set: no-op (the value is already on the stack -- synchronous path)
4. `run_ready()` continues with the next fiber in the ready queue
5. When the host calls `resume(token, value)`, the parked fiber is found, the placeholder on its stack is replaced with `value`, and the fiber moves back to `ready_fibers`
6. Next `run_ready()` call picks it up

### Ni Source Side

```ni
// Script authors just use await on native calls
var response = await http_get("https://example.com/api")
print(response)

// If the native function is synchronous (returns Ready), await is a no-op
var x = await some_sync_native()  // works fine, no parking
```

## Fiber Scheduling

### spawn

`spawn` creates a new fiber from a function reference and adds it to the ready queue:

```ni
fun worker():
    print("working")

spawn worker    // creates fiber, returns FiberId as int
```

The main fiber continues immediately. Spawned fibers run when `run_ready()` processes the queue. All fibers are cooperative -- they run until they finish, yield, or park on an `await`.

### run_ready(delta_time)

Processes suspended fibers and drains the ready queue:

1. **Process suspended fibers**: decrement wait timers by `delta_time`; promote expired timers and yielded fibers (timer == 0) to the ready queue
2. Run the main fiber if it's ready
3. Drain `ready_fibers` round-robin: pop each fiber, run it until it finishes, parks, or suspends
4. Fibers that `yield` or `wait` move to `suspended_fibers`
5. Fibers that `await` a pending native move to `parked_fibers`
6. Return `VmStatus` indicating the overall state

### resume(token, value)

Finds the parked fiber associated with `token`, replaces the placeholder value on its stack, and moves it to the ready queue. Returns an error if the token doesn't match any parked fiber.

## IoProvider Trait

Optional trait for hosts that want lifecycle notifications for fiber I/O:

```rust
pub trait IoProvider {
    /// Called when a fiber parks on a pending native call.
    fn on_pending(&mut self, token: PendingToken, call: SubstrateCall);

    /// Called before a fiber begins executing in run_ready().
    fn on_fiber_start(&mut self, _fiber_id: FiberId) {}

    /// Called when a pending operation is cancelled (its fiber was cancelled).
    /// The host should clean up any in-flight I/O associated with the token.
    fn on_cancel(&mut self, _token: PendingToken) {}
}
```

Set via `vm.set_io_provider(Box::new(my_provider))`. The host drives resolution via `resume()` calls; `IoProvider` provides the notification hooks so the host knows when operations start and when they're cancelled.

## Debugger Integration

The `VmObserver` trait has three hooks for async fiber events:

```rust
pub trait VmObserver {
    // ... existing hooks ...

    /// Called when a fiber parks on an await (going to sleep)
    fn on_fiber_park(&mut self, fiber_id: FiberId, token: PendingToken) {
        // default: no-op
    }

    /// Called when a parked fiber is resumed with a value
    fn on_fiber_resume(&mut self, fiber_id: FiberId) {
        // default: no-op
    }

    /// Called when a fiber is cancelled.
    /// If the fiber was parked, `token` is the pending operation token.
    fn on_fiber_cancel(&mut self, fiber_id: FiberId, token: Option<PendingToken>) {
        // default: no-op
    }
}
```

`VmState` now includes:
- `fiber_id: FiberId` -- which fiber is being inspected
- `source_line: String` -- the source text of the current line

### What This Means for Castle

Castle's debugger UI can:

1. **Show fiber state** -- display all fibers (ready, parked, finished, cancelled) with their IDs
2. **Track async operations** -- when `on_fiber_park` fires, show which native call caused the park and its `PendingToken`
3. **Visualize resume** -- when `on_fiber_resume` fires, show the fiber waking up with the resolved value
4. **Handle cancellation** -- when `on_fiber_cancel` fires, mark the fiber as cancelled in the UI. If `token` is `Some`, the associated async operation was also cancelled via `IoProvider::on_cancel`
5. **Correlate with substrate calls** -- `on_substrate_call` fires before the native function runs, `on_fiber_park` fires if it returns `Pending`. Together they tell Castle "fiber 3 called `http_get('/status')` and is waiting for a response"

### Timeline View Data

For Castle's timeline/trace view, the sequence of events for an async call looks like:

```
[fiber 1] on_substrate_call("http_get", ["/status"])
[fiber 1] on_fiber_park(fiber_id=1, token=42)
  ... time passes, other fibers may run ...
[fiber 1] on_fiber_resume(fiber_id=1)
[fiber 1] on_substrate_return("http_get", {status: 200, body: "..."})
[fiber 1] on_line(next_line, ...)
```

## GC Safety

The garbage collector traces roots across all fiber queues:
- Current fiber's stack, frames, and open upvalues
- All fibers in `ready_fibers`
- All fibers in `suspended_fibers` (yield/wait)
- All fibers in `parked_fibers` (async I/O)
- Host-pinned `extra_roots`

This means values held by sleeping fibers are never collected while the fiber is alive.

## API Summary

### Vm Methods

| Method | Description |
|--------|-------------|
| `Vm::new()` | Create VM with default config |
| `Vm::with_config(config)` | Create VM with custom config |
| `vm.load(closure)` | Set up main fiber from compiled closure |
| `vm.run(source)` | Compile + run to completion (traditional path) |
| `vm.run_ready(delta_time)` | Process suspended fibers, drain ready queue, returns `VmStatus` |
| `vm.resume(token, value)` | Resolve a pending operation, wake parked fiber |
| `vm.cancel_fiber(fid)` | Cancel a fiber by ID, returns `bool`. Notifies observer and IoProvider |
| `vm.set_io_provider(provider)` | Set optional I/O polling provider |
| `vm.stats()` | Get runtime statistics |
| `vm.parked_count()` | Number of parked fibers |
| `vm.suspended_count()` | Number of suspended fibers (yield/wait) |
| `vm.current_fiber_id()` | FiberId of the currently running fiber |
| `vm.finished_fibers()` | Drain completed fibers |

### Re-exports from ni_vm

All new types are re-exported from the `ni_vm` crate root:

```rust
use ni_vm::{
    NativeResult, PendingToken, FiberId, FiberHandle,
    VmConfig, VmStatus, VmStats, SubstrateCall, IoProvider,
};
```

## Backward Compatibility

- `NativeResult` implements `From<Result<Value, String>>`, so existing native functions that return `Ok(value)` or `Err(msg)` can be converted with `.into()`
- `Vm::new()` and `vm.run()` work exactly as before -- the async machinery has zero overhead when not used
- The `await` keyword was already reserved; now it has runtime meaning
- `spawn` previously pushed a dummy value; now it creates a real fiber and pushes its `FiberId` as `Value::Int`
