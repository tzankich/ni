# Embedding & Sandboxing

## Design Philosophy

Ni is designed to be embedded in host applications. The host exposes its own API to scripts as global functions, objects, and native classes. These are not part of the Ni language itself -- they are provided by the host and accessed as globals.

Scripts should not crash the host. A bug in a script should log an error and recover gracefully.

## Registering Host Functions

The host application registers native functions that scripts can call:

```rust
// Rust host example (synchronous):
vm.register_native("my_function", 2, |args, heap, intern| -> NativeResult {
    NativeResult::Ready(Value::Int(42))
});

// Async-capable native function:
vm.register_native("http_get", 1, |args, heap, intern| -> NativeResult {
    let token = PendingToken::new();
    // Host records token + args for external I/O resolution
    NativeResult::Pending(token)
});
```

See [Async Execution Model](async.md) for the full async I/O lifecycle.

## Host Fail Policy

Uncaught `fail` propagates to the host. The host configures behavior at VM creation:

```rust
pub enum FailPolicy {
    Error,   // return Err(NiError) -- good for tests
    Log,     // log message, push none, continue -- good for embedded hosts
}
```

- **`FailPolicy::Error`** (default): uncaught fail returns an error to the host. Suitable for test frameworks and CLI tools.
- **`FailPolicy::Log`**: uncaught fail logs the message to output, pushes `none`, and continues execution. Suitable for embedded hosts where scripts should never crash the application.

This is a VM configuration, not a language change. No syntax impact.

## Sandbox Guarantees

User scripts (bytecode) run in a sandbox with these guarantees:

1. **No system access.** No file I/O, no network, no process spawning, no system calls.
2. **Memory bounded.** Configurable per-script memory ceiling (default 32MB). Exceeding it terminates the fiber.
3. **CPU bounded.** Per-frame instruction limit (default 100K). Exceeding it terminates the fiber with "Script timeout."
4. **No pointer arithmetic.** The VM never exposes raw pointers to scripts.
5. **No native code execution.** Scripts cannot load or execute arbitrary native code.
6. **Isolation.** Scripts cannot directly access each other's private state -- only through the host API (which enforces access rules).

## Host API as Security Boundary

All interaction with the outside world goes through host-registered APIs. The API is the security boundary:

- Scripts call registered functions -> host validates arguments and performs the action.
- The host can restrict API access per-context (e.g., a third-party plugin script might have reduced permissions vs. the application's own scripts).

## Hot Reloading

Ni provides a hot-reload API for injecting new source into a running VM. Globals persist across calls, so redefined functions and classes take effect immediately while state variables are preserved.

```rust
use ni_compiler::hot_reload;

// Initial load
let mut vm = Vm::new();
hot_reload(&mut vm, "var counter = 0\nfun greet(): return \"hello\"")?;

// Later -- redefine greet, counter is preserved
hot_reload(&mut vm, "fun greet(): return \"goodbye\"")?;

// With source root for import resolution
use ni_compiler::hot_reload_with_source_root;
hot_reload_with_source_root(&mut vm, source, PathBuf::from("./scripts"))?;
```

Key properties:
- **State preserved:** globals survive across reloads.
- **Error safe:** syntax or compile errors leave existing state intact.
- **Fresh compiler:** each call creates a new `Compiler`, so there's no stale immutable-binding tracking across reloads. This means the REPL and hot reload can freely redefine things.
