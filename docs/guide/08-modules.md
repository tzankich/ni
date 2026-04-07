# Modules

## One File = One Module

Every `.ni` file is automatically a module. The filename (minus `.ni`) is the module name.

```
project/
    main.ni
    helpers.ni
    models/
        user.ni
```

Here, `helpers.ni` is the `helpers` module and `models/user.ni` is the `models.user` module.

## Importing Modules

### Import the whole module

```ni
import helpers

helpers.do_something()
```

### Import with an alias

```ni
import models.user as u

var player = u.User("Arthur")
```

### Import specific names

```ni
from helpers import calculate, validate

var result = calculate(42)
validate(result)
```

### Import multiple names

```ni
from models.user import User, Admin, Guest
```

## Standard Library Modules

Ni ships with three built-in modules:

### math

```ni
import math

print(math.PI)              // 3.14159...
print(math.sqrt(16))        // 4.0
print(math.abs(-5))         // 5
print(math.min(3, 7))       // 3
print(math.max(3, 7))       // 7
print(math.clamp(15, 0, 10)) // 10
print(math.floor(3.7))      // 3.0
print(math.ceil(3.2))       // 4.0
```

Or import specific functions:

```ni
from math import sqrt, PI

var circumference = 2 * PI * radius
var diagonal = sqrt(width * width + height * height)
```

### random

```ni
import random

random.seed(42)                    // reproducible results
var n = random.int(1, 6)          // random int 1-6 inclusive
var f = random.float(0.0, 1.0)   // random float [0, 1)
var b = random.bool()              // true or false
var lucky = random.chance(0.3)     // true 30% of the time

var colors = ["red", "green", "blue"]
var pick = random.choice(colors)   // random element
random.shuffle(colors)             // shuffle in place
```

### time

```ni
import time

var start = time.now()          // seconds since epoch (float)
var ms = time.millis()          // milliseconds since epoch (int)

// Do some work...

var elapsed = time.since(start)  // seconds elapsed since start
print(`Took {elapsed} seconds`)

time.sleep(0.5)                   // sleep for 0.5 seconds
```

Or import specific functions:

```ni
from time import now, since

var start = now()
// ...
print(since(start))
```

## Visibility

All top-level declarations are public by default. Prefix with `_` to make something private:

```ni
// In helpers.ni:

fun public_helper():       // can be imported
    return _internal()

fun _internal():           // cannot be imported
    return 42
```

Private names (starting with `_`) cannot be imported by other modules.

## Circular Imports

Ni does not allow circular imports. If module A imports B and module B imports A, the compiler reports an error. This keeps your dependency graph clean and predictable.

If you need shared code between two modules, extract it into a third module that both can import.

## What's Next

Your code is organized into modules. Let's learn about [testing](09-testing.md) -- making sure it all works.
