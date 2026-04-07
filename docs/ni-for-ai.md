# Ni Language -- Compact Reference

> Optimized for LLM context windows. Complete language coverage, minimal prose.
> Ni is a general-purpose embeddable scripting language. Python-like syntax, Lua-like coroutines, indentation-based blocks (4 spaces, no tabs).

## Syntax Essentials

```ni
// Line comments only. No block comments.
// Blocks start with : and indent 4 spaces.
// File extension: .ni | Test files: .spec.ni
// Lines ending with an operator, comma, or dot auto-continue to next line.
// Newlines inside (), [], {} are always ignored.
```

## Types

```
int      64-bit signed        42, -7, 0xFF, 0b1010, 1_000_000
float    64-bit IEEE 754      3.14, 0.5
bool                          true, false
string   immutable UTF-8      "double", 'single', `interp {expr}`
none     absence of value     none
list     ordered, mutable     [1, 2, 3], []
map      ordered k/v          ["key": val], [:]
```

Falsy values: `false`, `none`, `0`, `0.0`, `""`, `[]`.

## Variables and Bindings

```ni
var x = 10                   // mutable variable
var x: int = 10              // optional type annotation
const MAX = 100              // immutable binding
x += 1                       // compound: += -= *= /= %=
// const x = 5 -- immutable (compile error if reassigned)
// var x = 5 -- mutable variable (can be reassigned with =)
// x = 5 -- reassignment of existing mutable variable
// fun/class/enum/import bindings are also immutable
// function params and for-loop vars are mutable; catch/match bindings are immutable
```

## Operators (by precedence, highest first)

```
() [] .              call, subscript, member
- not try fail       unary
* / %                multiplicative
+ -                  additive (+ also concatenates strings and lists)
.. ..=               range (exclusive, inclusive)
< > <= >=            comparison
== != is             equality, identity
and                  logical AND (short-circuit, returns determining value)
or                   logical OR  (short-circuit, returns determining value)
= += -= *= /= %=    assignment
```

## Strings

```ni
"hello world"                // regular string -- no interpolation
'{"key": "val"}'             // braces are literal in regular strings
`hello {name}`               // backtick string -- interpolation with {expr}
`value is {x + 1}`           // expressions inside {} are evaluated

// Multiline strings (preserve newlines and indentation)
"""triple double quotes"""   // multiline literal (no interpolation)
'''triple single quotes'''   // multiline literal (no interpolation)
```triple backticks```       // multiline interpolated ({expr} evaluated)

"ha" * 3                     // "hahaha" (repetition)
"hello" + " world"           // concatenation
"hello"[0]                   // "h" (indexing)
"hello"[-1]                  // "o" (negative indexing)
// Regular string escapes: \n \t \r \\ \" \' \` \{ \} \0
// Backtick string escapes: \n \t \r \\ \` \{ \} \0
// Methods: .length .upper() .lower() .trim() .split(sep) .split(sep, limit)
//          .replace(old, new) .index_of(sub) .starts_with(s) .ends_with(s)
//          .contains(s) .slice(start, end) .char_at(i) .to_int() .to_float()
```

## Control Flow

```ni
if cond:                     // if / elif / else
    body
elif cond:
    body
else:
    body

while cond:                  // while loop
    body

for i in 0..10:              // for loop -- range (exclusive end)
for i in 0..=10:             // inclusive end
for item in list:            // iterate list
for k, v in map:             // iterate map

break                        // exit loop
continue                     // next iteration
pass                         // empty block placeholder

match value:                 // pattern matching
    when "a", "b":           // multiple patterns
        handle_ab()
    when x if x > 10:        // guard
        handle_large(x)
    when _:                  // wildcard (default)
        handle_other()
```

## Functions

```ni
fun name(a, b):              // declaration
    return a + b

fun name(a, b = 10):         // default parameter
    return a + b

var f = fun(x): x * 2        // lambda (anonymous)

fun outer():                 // closures capture enclosing vars
    var n = 0
    return fun():
        n += 1
        return n
```

Functions are first-class values. Without explicit `return`, functions return `none`.

## Collections

```ni
// Lists
var ls = [1, 2, 3]
ls[0]                        // 1 (0-indexed)
ls[-1]                       // 3 (negative indexing)
ls.add(4)                    // append
ls.pop()                     // remove and return last
ls.length                    // property, not method
ls.contains(2)               // true
ls.index_of(2)               // 1 (-1 if not found)
ls.join(", ")                // "1, 2, 3" (join elements with separator)
ls.sort()                    // sort in place
ls.reverse()                 // reverse in place
[1, 2] + [3, 4]             // [1, 2, 3, 4] (concatenation)
3 in ls                      // true (membership test)

// Maps
var m = ["hp": 100, "mp": 50]
m["hp"]                      // 100
m.hp                         // 100 (dot access for string keys)
m["str"] = 15                // add/update
m.keys()                     // list of keys
m.values()                   // list of values
"hp" in m                    // true (key check)

// Ranges
0..5                         // 0,1,2,3,4
0..=5                        // 0,1,2,3,4,5
```

## Classes

```ni
class Name:
    var field = default        // field with default

    fun init(params):         // constructor (called on Name(args))
        self.field = value

    fun method():             // instance method
        return self.field

class Child extends Parent:   // single inheritance
    fun init():
        super.init()          // call parent constructor
    fun method():             // override
        super.method()        // call parent method

var obj = Name(args)          // instantiation
obj.field                     // field access
obj.method()                  // method call
```

## Enums

```ni
enum Direction:
    north
    south
    east = 10                 // explicit value
    west = 20

var d = Direction.north
```

## Error Handling

```ni
try:                         // try/catch
    risky()
catch e:                     // e is optional
    print(e)

fail "message"               // throw any value (string, int, list, map, etc.)
fail 404

try:                         // catch-as-match
    operation()
catch e:
    when "not_found":
        fallback()
    when _:
        print(e)

var x = try risky()           // try as expression: returns none on fail
var x = try risky() ?? def    // combine with none-coalescing

assert condition             // fails if falsy
assert condition, "message"  // with message
```

## Modules

```ni
import math                          // import module
import models.user as u              // with alias
from math import sqrt, PI            // specific names
from utils import *                  // all (discouraged)
```

Each `.ni` file = one module. `_`-prefixed names are private (cannot be imported). Circular imports are a compile error.

### Standard Library

```ni
// math module
math.abs(x) math.min(a,b) math.max(a,b) math.clamp(x,lo,hi)
math.floor(x) math.ceil(x) math.round(x) math.sqrt(x) math.pow(b,e)
math.sin(x) math.cos(x) math.atan2(y,x) math.lerp(a,b,t)
math.PI math.TAU

// random module
random.int(min,max) random.float(min,max) random.bool() random.chance(p)
random.choice(list) random.shuffle(list) random.seed(n)

// time module
time.now()            // float seconds since epoch
time.millis()         // int milliseconds since epoch
time.since(start)     // float seconds elapsed since start
time.sleep(secs)      // sleep for secs seconds (rejects negative)

// json module
json.parse(str)       // parse JSON string → map/list/string/int/float/bool/none
json.encode(val)      // encode Ni value → JSON string
```

## Testing

```ni
spec "name":                 // top-level declaration
    assert condition
    assert condition, "msg"
    assert x == 5            // rich output: expected/but-was on failure

// spec blocks are stripped by `ni run` (zero cost)
// spec blocks compile and execute under `ni test`
```

### Structured Specs (BDD)

```ni
spec "checkout flow":
    given "a cart with items":       // setup context
        var cart = Cart()
        cart.add("widget", 10)

    when "user checks out":          // action
        cart.checkout()

        then "total is correct":     // assertion (each then = separate test)
            assert cart.total == 10

        when "discount applied":     // nested action
            cart.apply_discount("SAVE5")

            then "discount reflected":
                assert cart.total == 5
```

Each root-to-leaf path runs in isolation. `given`/`when` blocks re-execute per `then`.

### Data-Driven Specs

```ni
spec "validation" each (
    ["input": "",    "valid": false],
    ["input": "abc", "valid": true]
):
    given "input data":
        then "{input} validity is {valid}":
            assert validate(input) == valid
```

`each` binds map keys as locals. Labels are backtick strings (interpolated).

```bash
ni test              # discover + run all tests recursively
ni test file.ni      # run tests in file
ni test dir/         # run tests in directory
```

Test discovery: scans `.ni` files for lines starting with `spec "`. `.spec.ni` files always included. Each spec runs in a fresh fiber (isolated locals, shared top-level definitions).

## Coroutines

```ni
wait seconds                 // suspend fiber for N seconds (keyword, not function)
yield                        // suspend, host resumes
yield value                  // suspend with value

spawn func_ref               // create fiber from function, returns fiber id (int)
```

Fiber states: `CREATED → RUNNING ↔ SUSPENDED (yield/wait) or PARKED (async) → FINISHED | CANCELLED`

## Async / Await

```ni
// await pauses a fiber until the host resolves the async operation
var result = await http_get("/api/status")

// if the native function is synchronous, await is a no-op
var x = await sync_function()
```

Native functions return `NativeResult`: `Ready(value)`, `Pending(token)`, or `Error(msg)`.
When `Pending` is returned and `await` executes, the fiber parks. The host calls
`vm.resume(token, value)` when I/O completes. See `docs/internals/async.md` for details.

## Safe Navigation

```ni
obj?.field                   // none if obj is none
obj?.method()                // none if obj is none
a ?? b                       // b if a is none (none-coalescing)
obj?.field ?? default        // chain them
```

## Built-in Functions

```ni
print(value)                 // output to console
len(collection)              // length of list/map/string
type(value)                  // type name as string
```

## CLI

```bash
ni run <file>        # execute program
ni test [file|dir]   # run tests
ni repl              # interactive REPL
ni fmt <file>        # format source
ni lint <file>       # lint source
```

## Grammar (EBNF, abridged)

```ebnf
program     = { declaration } EOF
declaration = class_decl | fun_decl | var_decl | const_decl
            | enum_decl | import_decl | spec_decl | statement
class_decl  = "class" ID ["extends" ID] ":" INDENT {class_member} DEDENT
fun_decl    = "fun" ID "(" [params] ")" ["->" type] ":" block
var_decl    = "var" ID [":" type] "=" expr NL
const_decl  = "const" ID "=" expr NL
enum_decl   = "enum" ID ":" INDENT {ID ["=" expr] NL} DEDENT
spec_decl   = "spec" STRING [each_clause] ":" ( block | spec_body )
spec_body   = INDENT {spec_section} DEDENT
spec_section = ("given"|"when"|"then") STRING ":" INDENT {decl|spec_section} DEDENT
import_decl = "import" path ["as" ID] | "from" path "import" names
statement   = if_stmt | while_stmt | for_stmt | match_stmt
            | return_stmt | break | continue | pass
            | try_stmt | fail_stmt | assert_stmt | expr NL
block       = INDENT {declaration} DEDENT
```

## Keywords

```
var const fun class extends enum import from as return spec static get set
given when then each
if elif else for in while break continue match pass
yield wait spawn
try catch fail assert
and or not is true false none self super
case trait abstract private defer async await type       // reserved
```

## Naming Conventions (enforced by linter)

```
snake_case           variables, functions, parameters, const bindings
PascalCase           classes, enums
```

## Key Design Decisions

- Indentation: 4 spaces only, tabs are a compile error
- Integer division: `7 / 2 = 3` (use `7.0 / 2` for float division)
- No semicolons, no braces, no `def`/`function` -- just `fun`
- `const`/`var` for declaration, `=` for both binding and reassignment; mutability enforced at compile time
- `self` is explicit in method bodies, implicit in parameter lists
- `init` is the constructor name (not `__init__`)
- Single inheritance only (`extends`), no interfaces/traits
- `and`/`or` return the determining value (like Python)
- Sandboxed: no file I/O, no network, no system calls
- Strings are immutable, lists and maps are mutable
