# Getting Started

## Installing Ni

Ni is distributed as a single binary called `ni`. Download it for your platform from [nibang.com](https://nibang.com) or build from source:

```bash
git clone https://github.com/user/ni.git
cd ni
cargo build --release
# Binary is at target/release/ni
```

## Your First Program

Create a file called `hello.ni`:

```ni
print("Hello, World!")
```

Run it:

```bash
ni run hello.ni
```

Output:

```
Hello, World!
```

That's it. No boilerplate, no imports, no main function.

## The REPL

For quick experiments, use the interactive REPL:

```bash
ni repl
```

```
ni> print("Hello!")
Hello!
ni> 2 + 2
4
ni> var name = "Arthur"
ni> print(`I am {name}!`)
I am Arthur!
```

Type expressions to see their values. Type statements to execute them. Press Ctrl+D to exit.

## How Ni Code Looks

Ni uses indentation to define blocks, like Python. No curly braces, no semicolons. A colon (`:`) at the end of a line starts a new block, indented 4 spaces:

```ni
if health > 0:
    print("Still alive!")
    fight()
else:
    print("Game over")
```

Comments start with `//`:

```ni
// This is a comment
var score = 0    // This is also a comment
```

## Running Programs vs Tests

`ni run` executes your program. `ni test` runs any test blocks in your code:

```ni
fun double(x):
    return x * 2

// This test block is ignored by 'ni run'
// It only runs when you use 'ni test'
spec "double works":
    assert double(5) == 10
```

```bash
ni run my_file.ni     # runs the program (test blocks are skipped)
ni test my_file.ni    # runs only the test blocks
```

## File Organization

Ni files use the `.ni` extension. Each file is a module. A typical project might look like:

```
my_project/
    main.ni
    helpers.ni
    models/
        user.ni
        data.ni
```

## What's Next

Now that you can run programs, let's learn about [variables and types](02-variables-and-types.md).
