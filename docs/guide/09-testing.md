# Testing

## Built-in Tests

Ni has a built-in test framework. Write `spec` blocks right next to your code:

```ni
fun add(a, b):
    return a + b

spec "add returns the sum":
    assert add(2, 3) == 5

spec "add handles negatives":
    assert add(-1, 1) == 0
```

A spec block is a top-level declaration with a name (in quotes) and an indented body. Inside the body, use `assert` to check conditions.

## Running Tests

```bash
ni test                  # run all tests in current directory (recursive)
ni test my_file.ni       # run tests in one file
ni test tests/           # run tests in a directory
```

Example output:

```
my_file.ni
  PASS  add returns the sum
  PASS  add handles negatives

Results: 2 passed, 0 failed, 2 total
```

## Zero Cost in Production

When you run your program with `ni run`, spec blocks are completely ignored -- the compiler strips them out. Zero overhead, zero bytecode. Specs only compile and run when you use `ni test`.

```ni
// This spec block doesn't exist in production:
spec "sanity check":
    assert 1 + 1 == 2
```

This means you can keep specs right next to the code they test, without worrying about performance.

## Assert

`assert` checks that a condition is true. If it's false, the test fails:

```ni
spec "basic assertions":
    assert true
    assert 1 + 1 == 2
    assert "hello".length == 5
```

Add a message for clarity:

```ni
spec "health stays positive":
    var hp = calculate_health()
    assert hp >= 0, "health should never be negative"
```

When a test fails, you see the message and the line number:

```
  FAIL  health stays positive
        health should never be negative
        at my_file.ni:42
```

### Rich Assert Output

When an `assert` uses a comparison operator (`==`, `!=`, `<`, `>`, `<=`, `>=`), the failure message automatically shows the expected and actual values:

```ni
spec "damage calculation":
    var damage = calculate_hit(weapon, armor)
    assert damage == 25
```

```
  FAIL  damage calculation
        assert damage == 25
        expected: 25
         but was: 18
        at combat.ni:12
```

No special assertion functions needed -- plain `assert` with a comparison gives you rich output for free. If you provide a custom message, it's used instead.

## Test Isolation

Each test runs independently. Local variables in one test don't affect another:

```ni
const shared_function_works = true    // top-level: shared

spec "test A":
    var x = 1        // local to test A
    assert x == 1

spec "test B":
    // x doesn't exist here -- test B has its own locals
    var x = 999
    assert x == 999
```

Top-level definitions (functions, classes, immutable bindings) are shared across all tests. This is how tests access the code they're testing.

## Testing Classes

```ni
class Stack:
    var items = []

    fun init():
        self.items = []

    fun push(value):
        self.items = self.items + [value]

    fun pop():
        var last = self.items[-1]
        self.items = self.items[0..-1]
        return last

    fun is_empty():
        return self.items.length == 0

spec "new stack is empty":
    var s = Stack()
    assert s.is_empty()

spec "push and pop":
    var s = Stack()
    s.push(1)
    s.push(2)
    assert s.pop() == 2
    assert s.pop() == 1
    assert s.is_empty()

spec "stack tracks size":
    var s = Stack()
    s.push("a")
    s.push("b")
    s.push("c")
    assert s.items.length == 3
```

## Testing Error Cases

Use `try` to test that code fails correctly:

```ni
fun safe_divide(a, b):
    if b == 0:
        fail "division by zero"
    return a / b

spec "dividing by zero fails":
    var result = try safe_divide(10, 0)
    assert result == none

spec "normal division works":
    assert safe_divide(10, 2) == 5
```

## Test File Conventions

- Any `.ni` file can contain tests
- Files ending in `.spec.ni` are always treated as test files
- You can organize tests however you like:
  - Tests alongside code (recommended for small projects)
  - Separate `tests/` directory (recommended for larger projects)

```
project/
    calculator.ni              # has inline tests
    calculator.spec.ni         # or separate test file
    tests/
        test_integration.ni    # or in a tests directory
```

## A Real-World Example

From the Holy Grail quest -- 29 tests covering multiple classes and functions:

```ni
class Bridgekeeper:
    var questions_asked = 0

    fun init():
        self.questions_asked = 0

    fun ask(answer, correct_answer):
        self.questions_asked = self.questions_asked + 1
        if answer == correct_answer:
            return "pass"
        return "death"

    fun crossed():
        return self.questions_asked >= 3

spec "correct answer lets you pass":
    var bridge = Bridgekeeper()
    assert bridge.ask("Arthur", "Arthur") == "pass"

spec "wrong answer means death":
    var bridge = Bridgekeeper()
    assert bridge.ask("I dunno", "Arthur") == "death"

spec "bridgekeeper tracks questions":
    var bridge = Bridgekeeper()
    bridge.ask("a", "a")
    bridge.ask("b", "b")
    bridge.ask("c", "c")
    assert bridge.crossed()
```

## Structured Specs (given / when / then)

For complex scenarios, you can structure specs with `given`, `when`, and `then` blocks:

```ni
spec "user authentication":
    given "a registered user":
        var user = User("alice", "secret123")
        var auth = AuthService()

    when "they log in with correct password":
        var result = auth.login("alice", "secret123")

    then "they get a session":
        assert result != none
        assert result.user == "alice"

    then "the session is active":
        assert result.is_active()
```

Each `then` block is a separate test. The `given` and `when` blocks re-execute for each `then`, giving you full isolation without setup/teardown boilerplate.

### Nested Contexts

Blocks can nest to model branching scenarios:

```ni
spec "shopping cart":
    given "a cart with items":
        var cart = Cart()
        cart.add("widget", 10)
        cart.add("gadget", 25)

    when "the user checks out":
        cart.checkout()

        then "total is calculated":
            assert cart.total == 35

        when "a discount code is applied":
            cart.apply_discount("SAVE10")

            then "the discount is reflected":
                assert cart.total == 25
```

This produces two test paths: `cart → checkout → total` and `cart → checkout → discount → reflected`. Each runs from scratch.

### Data-Driven Specs (each)

Use `each` to run the same spec against multiple data sets:

```ni
spec "password validation" each (
    ["password": "abc",       "valid": false],
    ["password": "abc12345",  "valid": true],
    ["password": "",          "valid": false]
):
    given "a password":
        then `password '{password}' validity is {valid}`:
            assert validate(password) == valid
```

`each` iterates over a list of maps, binding each key as a local variable. The spec labels are backtick strings that interpolate the current values, so failures tell you exactly which row failed.

### Failure Output

On failure, the runner prints the full path with breadcrumb context:

```
FAIL  shopping cart
      given  a cart with items
       when  the user checks out
       when  a discount code is applied
       then  the discount is reflected

      assert cart.total == 25
      expected: 25
       but was: 35
      at shop_test.ni:14
```

The breadcrumb trail shows exactly which path through the spec tree failed. For `each` specs, the failing row's values are shown in the spec name.

## What's Next

Your code is tested. Let's learn about [coroutines](10-coroutines.md) -- Ni's killer feature for sequential-over-time logic.
