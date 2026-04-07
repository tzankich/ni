// Bug 2: Interned string IDs not remapped across module VM boundary
//
// When a module function's bytecode contains interned string references
// (e.g., map literal keys), the intern IDs from the module's temporary
// compilation VM are used as-is in the main VM. Since the two VMs have
// different intern tables, the IDs resolve to wrong strings.
//
// Root cause: compile_module_file() runs the module in a temporary VM,
// then deep-copies exported globals back to the caller's heap. The
// bytecode contains InternId references that index into the module VM's
// intern table, but these IDs are not remapped to the main VM's table.
//
// Expected output: ["key": "hello"]
// Actual: ["<wrong string>": "hello"]
//   The wrong string is whatever occupies that intern ID in the main VM.
//   If the ID exceeds the main VM's intern table size, it panics at
//   intern.rs:30 with "index out of bounds".

// Padding variables to prevent OOB crash (so we see corruption instead)
var a = "pad1"
var b = "pad2"
var c = "pad3"

from module_import_lib import make_map
print(make_map("hello"))
