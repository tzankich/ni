# Variables and Types

## Variables

Use `var name = value` to declare a mutable variable:

```ni
var name = "Arthur"
var health = 100
var alive = true
```

Variables must be initialized when declared. This prevents a whole class of "undefined variable" bugs.

You can change a variable's value with `=` (reassignment):

```ni
var score = 0
score = 10
score = score + 5    // score is now 15
score += 5           // shorthand, score is now 20
```

Compound assignment operators: `+=`, `-=`, `*=`, `/=`, `%=`.

## Immutable Bindings

Use `const name = value` to declare an immutable binding:

```ni
const max_health = 100
const pi = 3.14159
const title = "Holy Grail"
```

Immutable bindings cannot be reassigned -- the compiler enforces this at compile time.

The key distinction:
- `const x = 5` -- immutable binding (cannot be reassigned)
- `var x = 5` -- mutable variable (can be reassigned with `=`)
- `x = 5` -- reassignment of an existing mutable variable

Attempting to reassign an immutable binding produces a compile error:

```ni
const MAX = 100
MAX = 200    // Compile error: Cannot assign to immutable variable 'MAX'
```

Function, class, enum, and import bindings are also immutable:

```ni
fun greet():
    print("hello")

greet = 5       // Compile error: Cannot assign to immutable binding 'greet'
```

Function parameters and `for` loop variables are mutable. Catch variables and match bindings are immutable.

## Types

Ni has five basic types:

| Type | Examples | Description |
|------|----------|-------------|
| `int` | `42`, `0`, `-7` | Whole numbers |
| `float` | `3.14`, `0.5`, `-2.0` | Decimal numbers |
| `bool` | `true`, `false` | Yes or no |
| `string` | `"hello"`, `'world'` | Text |
| `none` | `none` | Absence of a value |

```ni
var age = 25              // int
var temperature = 98.6    // float
var active = true         // bool
var greeting = "Hello"    // string
var result = none         // none
```

### Integers

64-bit signed integers. You can use underscores for readability:

```ni
var million = 1_000_000
var hex_color = 0xFF00CC    // hex
var flags = 0b1010          // binary
```

### Floats

64-bit floating point numbers:

```ni
var pi = 3.14159
var ratio = 0.5
```

Integer division stays integer. Float division uses floats:

```ni
var a = 7 / 2       // 3 (integer division)
var b = 7.0 / 2     // 3.5 (float division)
```

### Strings

Strings use double or single quotes (they're identical):

```ni
var greeting = "Hello, World!"
var response = 'Ni!'
```

String interpolation uses backtick strings:

```ni
var name = "Arthur"
var hp = 100
print(`Knight {name} has {hp} HP`)
// Output: Knight Arthur has 100 HP
```

Any expression works inside `{...}`:

```ni
print(`2 + 2 = {2 + 2}`)
// Output: 2 + 2 = 4
```

Strings support concatenation and repetition:

```ni
var full = "Hello" + " " + "World"    // "Hello World"
var laugh = "ha" * 3                   // "hahaha"
```

Escape sequences: `\n` (newline), `\t` (tab), `\\` (backslash), `\"` (quote).

### Booleans

```ni
var alive = true
var game_over = false
```

### None

`none` represents "no value":

```ni
var target = none

if target == none:
    print("No target selected")
```

## Type Annotations (Optional)

You can add type annotations if you want. They're never required:

```ni
var name: string = "Arthur"
var health: int = 100
var ratio: float = 0.75
```

The compiler checks annotations when present but infers types when they're absent.

## Truthiness

In conditions, these values are "falsy" (treated as `false`):

- `false`
- `none`
- `0` (integer zero)
- `0.0` (float zero)
- `""` (empty string)
- `[]` (empty list)

Everything else is "truthy":

```ni
if "hello":
    print("non-empty strings are truthy")

if 0:
    print("this won't print")  // 0 is falsy
```

## Scope

Variables are visible from where they're declared to the end of their block:

```ni
var x = 1

if true:
    var y = 2       // y only exists in this block
    print(x)        // x is visible (outer scope)
    print(y)        // y is visible

// print(y)          // ERROR: y is not defined here
```

## What's Next

Now that you know how to store data, let's learn about [control flow](03-control-flow.md) -- making decisions and repeating actions.
