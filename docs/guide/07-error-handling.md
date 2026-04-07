# Error Handling

## The Basics

Things go wrong. Ni gives you tools to handle errors gracefully instead of crashing.

## Try / Catch

Wrap risky code in `try` and handle failures in `catch`:

```ni
try:
    var result = parse_data(raw_input)
    process(result)
catch error:
    print(`Something went wrong: {error}`)
```

If anything inside the `try` block fails, execution jumps to the `catch` block. The `error` variable contains whatever was thrown.

The catch variable is optional -- leave it off if you don't need the error value:

```ni
try:
    risky_operation()
catch:
    print("it failed, moving on")
```

## Fail

Use `fail` to signal an error. You can throw any value -- strings, numbers, lists, anything:

```ni
fun divide(a, b):
    if b == 0:
        fail "division by zero"
    return a / b
```

`fail` immediately exits the current function and unwinds the stack until a `try/catch` catches it. If nothing catches it, the program reports an error.

```ni
try:
    var result = divide(10, 0)
    print(result)               // never reached
catch e:
    print(e)                    // "division by zero"
```

Throw any type:

```ni
fail 404                          // integer
fail "not found"                  // string
fail ["code": 404, "msg": "nope"] // map
```

## Catch-as-Match

If your catch body starts with `when`, it pattern-matches on the error:

```ni
try:
    load_data(filename)
catch e:
    when "not_found":
        use_default_data()
    when "corrupted":
        print("Data corrupted, resetting")
        reset()
    when _:
        print(`Unexpected error: {e}`)
```

This is equivalent to a `match` statement inside the catch, but more concise.

## Try as an Expression

Use `try` as a prefix operator. It catches failures and returns `none`:

```ni
var data = try load_data("config.ni")

if data == none:
    print("Couldn't load config, using defaults")
```

Combine with `??` (none-coalescing) for one-line fallbacks:

```ni
var config = try load_config() ?? default_config()
var name = try get_user_name() ?? "Anonymous"
```

## Assert

`assert` checks a condition and fails if it's false:

```ni
assert health >= 0, "Health should never be negative"
assert items.length > 0
```

The message after the comma is optional. Assert is mainly used in tests:

```ni
spec "division works":
    assert divide(10, 2) == 5
    assert divide(9, 3) == 3

spec "division by zero fails":
    var result = try divide(10, 0)
    assert result == none
```

## Nested Try/Catch

Try blocks can be nested:

```ni
try:
    try:
        dangerous_inner()
    catch:
        print("inner failed, trying alternative")
        alternative_operation()
catch e:
    print(`everything failed: {e}`)
```

## Best Practices

1. **Catch specific errors** when you can, using catch-as-match
2. **Use `try` as expression** for simple fallbacks: `var x = try thing() ?? default`
3. **Fail with descriptive strings** so errors are easy to debug
4. **Don't catch everything** -- let unexpected errors propagate so you find bugs

## What's Next

You can handle errors gracefully. Let's learn about [modules](08-modules.md) -- organizing code across files.
