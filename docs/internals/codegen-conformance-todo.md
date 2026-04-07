# Codegen Conformance Testing

**Status:** Planned — not yet implemented
**Date noted:** 2026-04-07

## Problem

The Rust and C codegen backends can diverge from VM behavior in subtle ways.
Known examples found during code review (2026-04-07):

- `type_of()` returned title-case in codegen vs lowercase in VM (fixed)
- `print()` only printed its first argument in both backends (fixed)
- Static methods had wrong function signature in Rust backend (fixed)

These were found by manual review. There is no automated check that compiled
output matches VM output for the same Ni program.

## Proposed Approach

Dual-use the existing integration test suite (`crates/ni_repl/src/tests.rs`).

1. **Tag conformance-eligible tests.** Skip tests that use VM-only features
   (fibers, debugger, hot reload, instruction limits). The simple ones
   (arithmetic, strings, functions, classes, control flow, closures) are
   candidates.

2. **Add a `run_via_codegen(source)` helper** that:
   - Compiles Ni source to Rust via `ni_codegen`
   - Writes a temp `.rs` file with a `main()` wrapper
   - Invokes `rustc` to compile it
   - Runs the binary and captures stdout
   - Returns output lines (same shape as `run()`)

3. **A `run_conformance(source, expected)` wrapper** that calls both
   `run_expect()` and the codegen path, asserting identical output.

4. **Grow coverage gradually.** Start with the simplest passing tests,
   fix codegen bugs as they surface, expand the eligible set over time.

## Open Questions

- Should the C backend get the same treatment? (Needs a C compiler in CI.)
- How to handle Ni features the codegen doesn't support yet — skip gracefully
  or fail loudly?
- Should conformance tests run in CI on every PR, or as a separate scheduled job?
