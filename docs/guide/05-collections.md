# Collections

## Lists

An ordered collection of values:

```ni
var fruits = ["apple", "banana", "cherry"]
var numbers = [1, 2, 3, 4, 5]
var mixed = [1, "hello", true, none]     // any types
var empty = []
```

### Accessing Elements

Use zero-based indexing:

```ni
var colors = ["red", "green", "blue"]
print(colors[0])     // red
print(colors[2])     // blue
```

Negative indices count from the end:

```ni
print(colors[-1])    // blue (last element)
print(colors[-2])    // green
```

### Modifying Lists

```ni
var items = [1, 2, 3]

// Change an element
items[0] = 10          // [10, 2, 3]

// Add to the end
items.add(4)           // [10, 2, 3, 4]

// Remove last element
var last = items.pop() // last = 4, items = [10, 2, 3]

// Concatenate lists
var combined = [1, 2] + [3, 4]    // [1, 2, 3, 4]
```

### List Properties and Methods

```ni
var items = [3, 1, 4, 1, 5]

items.length           // 5
items.contains(4)      // true
items.index_of(4)      // 2
items.join(", ")       // "3, 1, 4, 1, 5" (join as string)
items.sort()           // sorts in place
items.reverse()        // reverses in place
```

### Iterating Lists

```ni
var fruits = ["apple", "banana", "cherry"]

for fruit in fruits:
    print(fruit)
```

## Maps

Key-value pairs (like dictionaries in Python):

```ni
var stats = ["hp": 100, "mp": 50, "str": 15]
var empty_map = [:]
```

Maps also support a brace-style literal. Quoted keys work, and bare
identifiers before `:` are treated as string keys (like JavaScript/Ruby):

```ni
var stats = {"hp": 100, "mp": 50}   // quoted keys
var stats = {hp: 100, mp: 50}       // bare identifier keys (same map)
```

If you need a computed/dynamic key, use the list form — inside `{}` a bare
identifier is always a literal string key, not a variable lookup:

```ni
const k = "hp"
var stats = [k: 100]                // key is the value of k
```

### Accessing Values

```ni
var stats = ["hp": 100, "mp": 50]

print(stats["hp"])     // 100
stats["str"] = 15      // add a new key
stats["hp"] = 90       // update existing
```

Maps also support dot access for string keys:

```ni
print(stats.hp)        // 100 (same as stats["hp"])
stats.hp = 90          // same as stats["hp"] = 90
```

### Map Methods

```ni
var m = ["a": 1, "b": 2, "c": 3]

m.length               // 3
m.keys()               // ["a", "b", "c"]
m.values()             // [1, 2, 3]
m.contains_key("a")    // true
```

### Iterating Maps

```ni
var stats = ["hp": 100, "mp": 50]

for key, value in stats:
    print(`{key} = {value}`)
// Output:
// hp = 100
// mp = 50
```

## Ranges

Ranges represent sequences of numbers:

```ni
0..5       // 0, 1, 2, 3, 4 (exclusive end)
0..=5      // 0, 1, 2, 3, 4, 5 (inclusive end)
```

Most commonly used in `for` loops:

```ni
for i in 0..5:
    print(i)    // 0, 1, 2, 3, 4

for i in 1..=10:
    print(i)    // 1, 2, 3, 4, 5, 6, 7, 8, 9, 10
```

## The `in` Operator

Check if something is in a collection:

```ni
var items = [1, 2, 3]
print(3 in items)          // true
print(5 in items)          // false

var stats = ["hp": 100]
print("hp" in stats)       // true

print("lo" in "hello")     // true (substring check)
```

## Putting It Together

A practical example combining collections:

```ni
class Inventory:
    var items = []

    fun init():
        self.items = []

    fun add(item):
        self.items = self.items + [item]

    fun has(item):
        for i in self.items:
            if i == item:
                return true
        return false

    fun count():
        return self.items.length

spec "inventory management":
    var inv = Inventory()
    inv.add("sword")
    inv.add("shield")
    assert inv.count() == 2
    assert inv.has("sword")
    assert not inv.has("potion")
```

## What's Next

You know how to store and organize data. Let's learn about [classes](06-classes.md) -- creating your own types.
