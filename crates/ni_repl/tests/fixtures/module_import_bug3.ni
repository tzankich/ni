// Bug 3: Intern table index out of bounds (same root cause as Bug 2)
//
// When the module's intern IDs exceed the main VM's intern table length,
// the VM panics instead of returning a graceful error.
//
// Expected output: ["key": "hello"]
// Actual: thread panicked at crates/ni_vm/src/intern.rs:30:24:
//         index out of bounds: the len is N but the index is N+M

from module_import_lib import make_map
print(make_map("hello"))
