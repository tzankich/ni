# The Ni Programming Language

> *"We are the Knights Who Say... Ni!"*

Ni is a general-purpose embeddable scripting language. It combines Python's readability with Lua's lightweight coroutine model in a clean, modern package.

```ni
fun greet(name):
    return `Hello, {name}!`

class Counter:
    var count = 0
    fun increment():
        self.count += 1
    fun value():
        return self.count

spec "counter works":
    var c = Counter()
    c.increment()
    c.increment()
    assert c.value() == 2
```

## Why Ni?

- **Beginner-first.** Python-style syntax, indentation blocks, no semicolons, no braces. If you can read Python, you can read Ni.
- **Embeddable.** Designed to be the scripting layer inside applications. Sandboxed, memory-bounded, no system access.
- **Coroutines built in.** `wait 0.5` suspends and resumes. No callbacks, no promises, no boilerplate.
- **Built-in testing.** Write `spec` blocks right next to your code. Run them with `ni test`.
- **Multiple backends.** Same source compiles to bytecode (VM), Rust, or C.

## Documentation

### [Learn Ni](guide/01-getting-started.md) -- Tutorial Guide

A hands-on tutorial that takes you from zero to productive. No prior programming experience required.

1. [Getting Started](guide/01-getting-started.md) -- Install, run your first program
2. [Variables and Types](guide/02-variables-and-types.md) -- Data, names, and values
3. [Control Flow](guide/03-control-flow.md) -- Decisions and loops
4. [Functions](guide/04-functions.md) -- Reusable code
5. [Collections](guide/05-collections.md) -- Lists, maps, ranges
6. [Classes](guide/06-classes.md) -- Objects and inheritance
7. [Error Handling](guide/07-error-handling.md) -- Try, catch, fail
8. [Modules](guide/08-modules.md) -- Organizing code across files
9. [Testing](guide/09-testing.md) -- Built-in test framework
10. [Coroutines](guide/10-coroutines.md) -- Concurrent sequential logic

### [Internals](internals/) -- Technical Reference

For language implementors and power users. Covers the [bytecode VM](internals/vm.md), [async execution model](internals/async.md), [native code generation](internals/codegen.md), [embedding & sandboxing](internals/embedding.md), [formal grammar](internals/grammar.md), and [roadmap](internals/roadmap.md).

### [Ni for AI](ni-for-ai.md) -- Compact Reference

The entire language in one dense file, optimized for LLM context windows. If you're an AI assistant helping someone write Ni code, start here.

## Quick Start

```bash
# Run a program
ni run hello.ni

# Start the REPL
ni repl

# Run tests
ni test                  # all tests in current directory
ni test my_file.ni       # tests in a specific file

# Format code
ni fmt my_file.ni

# Lint code
ni lint my_file.ni
```

## Project Links

- **Website:** nibang.com
- **Internals:** [internals/](internals/)
- **Examples:** [examples/](../examples/)
