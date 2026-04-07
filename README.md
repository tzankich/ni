# Ni Scripting Language

> *"We are the Knights Who Say... Ni!"*

Ni is a general-purpose embeddable scripting language. It combines Python's readability with a lightweight coroutine model in a clean, modern package.

```ni
// Classes with inheritance
class Quest:
    var name
    var knights = []

    fun add(knight):
        self.knights.push(knight)
        return self

    fun embark():
        var party = self.knights.join(", ")
        return `{party} ride forth on the quest for {self.name}!`

// Coroutines -- sequential-over-time logic reads like normal code
fun patrol(guard):
    while true:
        guard.walk_to("north gate")
        wait 2.0
        guard.walk_to("south gate")
        wait 2.0

// Pattern matching
fun describe(item):
    match item:
        ["Holy Hand Grenade", _]:
            return "Count to three. Not five."
        [name, weight] if weight > 10:
            return `{name} is too heavy to carry.`
        [name, _]:
            return `You pick up the {name}.`

// Built-in testing -- specs live right next to the code
spec "quest assembly":
    var q = Quest(name: "the Holy Grail")
    q.add("Arthur").add("Lancelot").add("Robin")
    assert q.knights.length() == 3
    assert q.embark().contains("Holy Grail")

spec "pattern matching":
    assert describe(["Holy Hand Grenade", 2]) == "Count to three. Not five."
    assert describe(["boulder", 50]).contains("too heavy")
    assert describe(["shrubbery", 3]).contains("pick up")
```

## Features

- **Python-style syntax** -- indentation blocks, no semicolons, no braces
- **Embeddable** -- sandboxed, memory-bounded, designed as a scripting layer inside host applications
- **Coroutines** -- `wait 0.5` suspends and resumes; sequential-over-time code reads like normal sequential code
- **Built-in testing** -- write `spec` blocks alongside your code, run with `ni test`
- **Multiple backends** -- bytecode VM, Rust, or C
- **Classes and enums** -- single inheritance, pattern matching with guards
- **Modules** -- `from math_utils import sqrt`
- **String interpolation** -- `` `Hello, {name}!` ``
- **LSP support** -- completions, go-to-definition, hover, diagnostics

## Testing

Ni has a built-in BDD test framework. Specs live alongside your code -- no separate test files, no test runner config.

```ni
spec "the knights who say ni":
    given "a quest for shrubberies":
        var cart = Cart()
        cart.add("shrubbery", 5)

    when "the knights demand another":
        cart.add("slightly higher shrubbery", 8)

        then "we have a two-level effect":
            assert cart.items.length() == 2
            assert cart.total == 13

        when "we add a path down the middle":
            cart.add("nice little path", 3)

            then "the knights are satisfied":
                assert cart.total == 16
```

Each `then` is a separate test. The `given`/`when` blocks re-execute for each `then` -- full isolation, no setup/teardown boilerplate.

When tests pass, silence:

```
$ ni test shrubbery.ni
2 passed
```

When they fail, a breadcrumb trail shows exactly which path broke and why:

```
FAIL  the knights who say ni
      given  a quest for shrubberies
       when  the knights demand another
       when  we add a path down the middle
       then  the knights are satisfied

      assert cart.total == 16
      expected: 16
       but was: 13
      at shrubbery.ni:14
```

Data-driven specs test multiple cases with `each`:

```ni
spec "password rules" each (
    ["pw": "abc",       "ok": false],
    ["pw": "abc12345",  "ok": true],
    ["pw": "",          "ok": false]
):
    then `'{pw}' valid={ok}`:
        assert validate(pw) == ok
```

## Quick Start

```bash
# Build from source
cargo build --release

# Run a program
ni run hello.ni

# Start the REPL
ni repl

# Run tests
ni test                  # all tests in current directory
ni test my_file.ni       # tests in a specific file

# Format and lint
ni fmt my_file.ni
ni lint my_file.ni
```

## Embedding in Rust

Ni is designed to be embedded. The VM is a library crate (`ni_vm`) with a straightforward API:

```rust
use ni_vm::{Vm, VmConfig};
use ni_compiler;

let config = VmConfig {
    instruction_limit: 1_000_000,
    memory_limit: Some(64 * 1024 * 1024),
    ..VmConfig::default()
};
let mut vm = Vm::with_config(config);

let closure = ni_compiler::compile_source(source, &mut vm.heap, &mut vm.interner)?;
vm.interpret(closure)?;

// Call a script function
let func = vm.get_global("my_function").unwrap();
let result = vm.call(func, &[Value::Int(42)])?;
```

Host applications can register native functions, control resource limits, and configure error handling policies. See the [embedding guide](docs/internals/embedding.md) for details.

## Project Structure

```
crates/
  ni_error/       Error types and source spans
  ni_lexer/       Tokenizer with indentation tracking
  ni_parser/      Recursive descent parser → AST
  ni_compiler/    AST → bytecode compiler
  ni_vm/          Bytecode VM with GC, coroutines, fibers
  ni_runtime/     Shared runtime types for native backends
  ni_codegen/     Rust and C code generation
  ni_fmt/         Code formatter
  ni_lint/        Linter
  ni_lsp/         Language Server Protocol implementation
  ni_repl/        Interactive REPL and CLI
docs/             Language guide and internals
examples/         Example scripts
editors/vscode/   VS Code extension (syntax highlighting + LSP)
```

## Documentation

- **[Language Guide](docs/guide/01-getting-started.md)** -- hands-on tutorial from zero to productive
- **[Ni for AI](docs/ni-for-ai.md)** -- the entire language in one file, optimized for LLM context windows
- **[Internals](docs/internals/)** -- VM architecture, async model, code generation, grammar

## License

MIT
