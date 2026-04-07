// Bug 1: Cross-function references cause stack overflow
//
// When a module exports function A that calls function B (both defined
// in the same module), calling A from the importing script causes a
// stack overflow instead of properly dispatching to B.
//
// Expected output: 6
// Actual: RuntimeError: Stack overflow

from module_import_lib import use_helper
print(use_helper(5))
