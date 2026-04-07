# Control Flow

## If / Elif / Else

Make decisions with `if`:

```ni
var health = 15

if health <= 0:
    print("You are dead")
elif health < 20:
    print("Low health!")
else:
    print("Doing fine")
```

`elif` is short for "else if." You can chain as many as you need. The `else` block runs if nothing else matched.

### Logical Operators

Combine conditions with `and`, `or`, and `not`:

```ni
if health > 0 and has_weapon:
    attack()

if is_dead or is_stunned:
    skip_turn()

if not game_over:
    continue_playing()
```

`and` and `or` short-circuit -- they stop evaluating as soon as the result is known:

```ni
// safe_divide won't be called if x is 0
var result = x != 0 and safe_divide(100, x)
```

They also return the determining value, which is useful for defaults:

```ni
var name = user_input or "Unknown"    // "Unknown" if user_input is falsy
```

## While Loops

Repeat while a condition is true:

```ni
var count = 0
while count < 5:
    print(count)
    count += 1
// Output: 0 1 2 3 4
```

## For Loops

Iterate over ranges, lists, maps, or any iterable:

```ni
// Count from 0 to 4
for i in 0..5:
    print(i)

// Inclusive range: 0 to 5
for i in 0..=5:
    print(i)

// Iterate a list
var fruits = ["apple", "banana", "cherry"]
for fruit in fruits:
    print(fruit)

// Iterate a map
var stats = ["hp": 100, "mp": 50]
for key, value in stats:
    print(`{key} = {value}`)
```

## Break and Continue

`break` exits a loop immediately. `continue` skips to the next iteration:

```ni
for i in 0..100:
    if i == 5:
        break           // stop the loop entirely
    print(i)
// Output: 0 1 2 3 4

for i in 0..10:
    if i % 2 == 0:
        continue        // skip even numbers
    print(i)
// Output: 1 3 5 7 9
```

## Match (Pattern Matching)

Match a value against multiple patterns:

```ni
var command = "north"

match command:
    when "north":
        move(0, -1)
    when "south":
        move(0, 1)
    when "east":
        move(1, 0)
    when "west":
        move(-1, 0)
    when _:
        print(`Unknown command: {command}`)
```

The `_` pattern matches anything -- it's the default branch.

### Multiple Patterns

Match several values in one branch:

```ni
match input:
    when "up", "w", "north":
        go_north()
    when "down", "s", "south":
        go_south()
    when "quit", "exit", "q":
        shutdown()
```

### Match with Guards

Add `if` conditions to patterns:

```ni
match damage:
    when d if d >= 100:
        print("CRITICAL HIT!")
    when d if d > 0:
        print(`Hit for {d} damage`)
    when 0:
        print("Miss!")
```

## Pass

A placeholder for empty blocks (identical to Python):

```ni
if debug_mode:
    pass    // TODO: add debug overlay later
```

## Line Continuation

When a line ends with a binary operator, comma, or dot, the next line is automatically a continuation -- no special syntax needed:

```ni
var result = very_long_value +
    another_value +
    final_value

if some_condition and
    another_condition:
    do_stuff()

// Method chaining across lines
items.
    filter(fun(x): x > 0).
    map(fun(x): x * 2)
```

This works with all binary operators (`+`, `-`, `/`, `%`, `==`, `!=`, `<`, `>`, `<=`, `>=`), logical operators (`and`, `or`), assignment operators (`=`, `+=`, `-=`, `*=`, `/=`, `%=`), comma, dot, and keyword operators (`in`, `is`).

You can also wrap expressions in parentheses for multi-line -- newlines inside `()`, `[]`, and `{}` are always ignored:

```ni
var x = (
    1 +
    2 +
    3
)
```

## What's Next

You can make decisions and repeat actions. Now let's learn about [functions](04-functions.md) -- packaging code for reuse.
