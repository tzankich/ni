# Functions

## Defining Functions

Use `fun` to define a function:

```ni
fun greet(name):
    print(`Hello, {name}!`)

greet("Arthur")    // Hello, Arthur!
```

Functions that compute a value use `return`:

```ni
fun add(a, b):
    return a + b

var result = add(3, 4)    // 7
```

A function without `return` (or with `return` and no value) returns `none`.

## Default Parameters

Give parameters default values:

```ni
fun heal(target, amount = 10):
    target.hp += amount

heal(player)          // heals 10
heal(player, 25)      // heals 25
```

Parameters with defaults must come after parameters without:

```ni
fun create_enemy(name, hp = 50, damage = 10):
    // ...
```

## Return Values

Functions return a single value:

```ni
fun max(a, b):
    if a > b:
        return a
    return b

print(max(3, 7))    // 7
```

Return early to simplify logic:

```ni
fun find_item(inventory, name):
    for item in inventory:
        if item.name == name:
            return item
    return none
```

## Functions as Values

Functions are first-class values. Store them in variables, pass them as arguments:

```ni
fun double(x):
    return x * 2

var operation = double
print(operation(5))    // 10
```

Pass functions to other functions:

```ni
fun apply(value, transform):
    return transform(value)

fun negate(x):
    return -x

print(apply(5, negate))    // -5
```

## Lambdas (Anonymous Functions)

Create small functions inline:

```ni
var double = fun(x): x * 2
print(double(5))    // 10
```

Lambdas are great as callbacks:

```ni
var numbers = [3, 1, 4, 1, 5]
numbers.sort(by = fun(x): x)
```

Multi-line lambdas use an indented block:

```ni
var process = fun(item):
    item.validate()
    item.save()
    return item.id
```

## Closures

Functions capture variables from their enclosing scope:

```ni
fun make_counter():
    var count = 0
    return fun():
        count += 1
        return count

var counter = make_counter()
print(counter())    // 1
print(counter())    // 2
print(counter())    // 3
```

Each call to `make_counter()` creates a fresh `count` variable. The returned function "closes over" it, keeping it alive.

## Type Annotations (Optional)

Add parameter types and return types for clarity:

```ni
fun distance(x: float, y: float) -> float:
    return math.sqrt(x * x + y * y)
```

These are always optional. The compiler checks them when present.

## What's Next

Functions work on individual values. Let's learn about [collections](05-collections.md) -- working with groups of data.
