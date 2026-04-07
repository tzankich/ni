# Coroutines

## Why Coroutines?

Many tasks are naturally sequential-over-time: "do this, wait, do that, wait, do the other thing." Without coroutines, this typically requires state machines, callbacks, or async/await. With Ni's coroutines, sequential-over-time code reads like normal sequential code:

```ni
fun patrol(guard):
    while true:
        guard.walk_to(checkpoint_a)
        wait 2.0                     // pause for 2 seconds
        guard.look_around()
        wait 1.0
        guard.walk_to(checkpoint_b)
        wait 2.0
        guard.look_around()
        wait 1.0
```

Each `wait` suspends the coroutine. The host application resumes it later. The code reads top-to-bottom, like prose.

## Wait

```ni
wait 1.5           // pause for 1.5 seconds
wait 0             // yield for one frame, resume next frame
```

`wait` is a keyword that pauses the current coroutine for the given number of seconds. It doesn't block anything else -- other coroutines and the host application keep running. The host calls `run_ready(delta_time)` each frame; suspended fibers with expired wait timers automatically resume.

## Spawning Fibers

A **fiber** is a running coroutine. Use `spawn` to create one:

```ni
fun count_up():
    for i in 1..=5:
        print(i)
        wait 1.0

var counter = spawn count_up()
```

The spawned fiber runs concurrently (cooperatively -- not in parallel threads). Currently `spawn` returns the fiber's ID as an integer. The host can cancel fibers via `vm.cancel_fiber(fid)`.

> **Planned**: Fiber objects with `.is_alive`, `.cancel()`, `.result` properties, and `wait_all()`/`wait_any()` built-in functions.

## Yield

`yield` is a lower-level primitive. It suspends the current fiber and lets the host decide when to resume:

```ni
fun producer():
    yield 1
    yield 2
    yield 3
```

In most cases, `wait` is what you want. `yield` is for advanced patterns like custom schedulers.

## Fiber Lifecycle

```
CREATED  →  RUNNING  →  (SUSPENDED ↔ RUNNING)  →  FINISHED
                                                  ↘ CANCELLED
```

- **CREATED**: Fiber exists but hasn't started
- **RUNNING**: Currently executing
- **SUSPENDED**: Paused at a `wait` or `yield`
- **FINISHED**: Reached the end or hit `return`
- **CANCELLED**: Stopped by `.cancel()`

## Parallel Execution

Spawn multiple fibers for concurrent work:

```ni
fun download(url):
    // simulated download
    wait 2.0
    print(`Downloaded {url}`)

// Start three downloads concurrently
spawn download("file1.dat")
spawn download("file2.dat")
spawn download("file3.dat")
```

## Practical Example: Dialogue System

Coroutines make dialogue systems trivial:

```ni
fun run_dialogue():
    show_text("Welcome, brave knight!")
    wait 2.0
    show_text("I am the keeper of the Bridge of Death.")
    wait 2.0
    show_text("Answer me these questions three...")
    wait 1.5

    var answer = ask_question("What is your name?")
    wait 0.5

    if answer == "Arthur":
        show_text("Very well, you may pass.")
    else:
        show_text("Wrong! Into the gorge with you!")

spawn run_dialogue()
```

No state tracking, no callbacks -- just top-to-bottom code.

## When to Use Coroutines

Coroutines shine for:
- **Timed sequences**: animations, cutscenes, tutorials
- **Polling**: check a condition periodically
- **Concurrent tasks**: multiple independent activities running together
- **Sequential async**: do A, wait, do B, wait, do C

They're one of Ni's most powerful features -- the thing that makes scripting feel natural instead of mechanical.

## Summary

| Feature | Description |
|---------|-------------|
| `wait seconds` | Pause the current fiber for N seconds |
| `yield` / `yield value` | Suspend the fiber, optionally with a value |
| `spawn func` | Create a new fiber, returns fiber ID |
| `vm.cancel_fiber(fid)` | Cancel a fiber by ID (host API) |

## What's Next

Congratulations! You've completed the Ni tutorial. You know:

- Variables and types
- Control flow
- Functions and closures
- Collections
- Classes and inheritance
- Error handling
- Modules
- Testing
- Coroutines

For the complete language details, see the [internals docs](../internals/). For a compact reference, see [Ni for AI](../ni-for-ai.md).

Happy coding! And remember: *"We are the Knights Who Say... Ni!"*
