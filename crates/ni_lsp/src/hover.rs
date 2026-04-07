use tower_lsp::lsp_types::*;

/// Return beginner-friendly hover content for a word at the cursor.
pub fn hover_for_word(word: &str) -> Option<Hover> {
    let content = match word {
        // Control flow
        "if" => "**if** -- Check a condition\n\nRuns the indented code only when the condition is true.\n```ni\nif score > 100:\n    print(\"You win!\")\n```",
        "elif" => "**elif** -- Check another condition\n\nShort for \"else if\". Checked only when the previous `if` was false.\n```ni\nif x > 10:\n    print(\"big\")\nelif x > 0:\n    print(\"small\")\n```",
        "else" => "**else** -- The fallback\n\nRuns when none of the `if`/`elif` conditions were true.\n```ni\nif raining:\n    print(\"Stay inside\")\nelse:\n    print(\"Go outside!\")\n```",
        "while" => "**while** -- Repeat while true\n\nKeeps running the indented code as long as the condition is true.\n```ni\nwhile lives > 0:\n    play_round()\n```",
        "for" => "**for** -- Loop through items\n\nRuns the code once for each item in a list or range.\n```ni\nfor i in 0..5:\n    print(i)\n```",
        "in" => "**in** -- Part of a `for` loop or membership test\n\nUsed with `for` to loop, or to check if something is inside a list.\n```ni\nfor item in inventory:\n    print(item)\n```",
        "break" => "**break** -- Exit a loop early\n\nStops the current `while` or `for` loop immediately.",
        "continue" => "**continue** -- Skip to next iteration\n\nJumps to the top of the loop and starts the next cycle.",
        "match" => "**match** -- Compare against patterns\n\nLike a multi-way `if`, but cleaner.\n```ni\nmatch color:\n    when \"red\":\n        print(\"Stop!\")\n    when \"green\":\n        print(\"Go!\")\n```",
        "when" => "**when** -- One branch of a `match`, or a context in a `spec`\n\nIn `match`: a pattern to compare against. In `spec`: a setup step between `given` and `then`.",
        "pass" => "**pass** -- Do nothing\n\nA placeholder when you need a block but have nothing to put in it yet.",
        "return" => "**return** -- Send a value back\n\nExits the current function and gives back a result.\n```ni\nfun double(n):\n    return n * 2\n```",

        // Declarations
        "var" => "**var** -- Create a mutable variable\n\nMakes a new variable you can change later.\n```ni\nvar score = 0\nscore = score + 10\n```",
        "fun" => "**fun** -- Define a function\n\nA reusable block of code with a name.\n```ni\nfun greet(name):\n    print(\"Hello, \" + name + \"!\")\n```",
        "class" => "**class** -- Define a class\n\nA blueprint for creating objects with shared behavior.\n```ni\nclass Player:\n    fun init(name):\n        self.name = name\n        self.hp = 100\n```",
        "extends" => "**extends** -- Inherit from another class\n\nMakes a new class that gets all the features of an existing one.\n```ni\nclass Boss extends Enemy:\n    fun init():\n        super.init(\"Boss\", 500)\n```",
        "enum" => "**enum** -- Define named constants\n\nA set of related values with names.\n```ni\nenum Direction:\n    north = 0\n    south = 1\n```",
        "import" => "**import** -- Load code from another file\n\nBrings in functions or classes from a different module.",
        "from" => "**from** -- Import specific items\n\nPick exactly which things to bring in from a module.\n```ni\nfrom math import sqrt, pi\n```",
        "as" => "**as** -- Rename an import\n\nGive an imported item a different name in your code.",
        "static" => "**static** -- Belongs to the class, not instances\n\nA field or method shared by all instances of a class.",
        "spawn" => "**spawn** -- Start a script in the background\n\nRuns a function as a fiber (like a background task) without blocking.\n```ni\nspawn patrol_route()\n```",
        "yield" => "**yield** -- Pause and resume later\n\nPauses the current fiber so other code can run.",
        "wait" => "**wait** -- Pause this script\n\nPause the current fiber without freezing the whole game.\n```ni\nwait 1.0  // wait 1 second\n```",
        "fiber" => "**fiber** -- A lightweight coroutine\n\nA script that can pause and resume, perfect for game AI and cutscenes.",

        // Operators
        "and" => "**and** -- Both must be true\n\n```ni\nif has_key and door_locked:\n    open_door()\n```",
        "or" => "**or** -- Either can be true\n\n```ni\nif health <= 0 or surrendered:\n    game_over()\n```",
        "not" => "**not** -- Flip true/false\n\n```ni\nif not game_over:\n    keep_playing()\n```",
        "is" => "**is** -- Check the type\n\n```ni\nif enemy is Boss:\n    run_away()\n```",

        // Values
        "true" => "**true** -- The boolean value true",
        "false" => "**false** -- The boolean value false",
        "none" => "**none** -- No value\n\nRepresents the absence of a value, like \"nothing here\".",
        "self" => "**self** -- The current object\n\nInside a method, `self` refers to the object the method was called on.\n```ni\nfun init(name):\n    self.name = name\n```",
        "super" => "**super** -- The parent class\n\nCall a method from the class you're extending.\n```ni\nsuper.init(\"Boss\", 500)\n```",

        // Error handling
        "try" => "**try** -- Handle errors safely\n\nRun code that might fail, and catch the error.\n```ni\ntry:\n    risky_operation()\ncatch e:\n    print(\"Something went wrong: \" + e)\n```",
        "catch" => "**catch** -- Handle an error\n\nThe code here runs when `try` encounters an error.",
        "fail" => "**fail** -- Trigger an error\n\nStop execution and report an error.\n```ni\nfail \"Something went wrong!\"\n```",
        "assert" => "**assert** -- Check that something is true\n\nIf the condition is false, stops with an error. Great for debugging.\n```ni\nassert score >= 0, \"Score can't be negative!\"\n```",

        // Built-in functions
        "print" => "**print(value)** -- Show text on screen\n\nDisplays a value. The most useful debugging tool!\n```ni\nprint(\"Hello!\")\nprint(42)\n```",
        "len" => "**len(collection)** → int\n\nReturns how many items are in a list, map, or string.\n```ni\nprint(len([1, 2, 3]))  // 3\n```",
        "type_of" => "**type_of(value)** → string\n\nTells you what type a value is.\n```ni\nprint(type_of(42))      // \"int\"\nprint(type_of(\"hello\")) // \"string\"\n```",
        "to_string" => "**to_string(value)** → string\n\nConverts any value to its text representation.",
        "to_int" => "**to_int(value)** → int\n\nConverts a string or float to an integer.",
        "to_float" => "**to_float(value)** → float\n\nConverts a string or integer to a decimal number.",
        "abs" => "**abs(number)** → number\n\nReturns the absolute (positive) value.\n```ni\nprint(abs(-5))  // 5\n```",
        "min" => "**min(a, b)** → value\n\nReturns the smaller of two values.",
        "max" => "**max(a, b)** → value\n\nReturns the larger of two values.",
        "range" => "**range(end)** or **range(start, end)** → list\n\nCreates a sequence of numbers.\n```ni\nfor i in range(5):\n    print(i)  // 0, 1, 2, 3, 4\n```",
        "input" => "**input(prompt)** → string\n\nAsk the user to type something.\n```ni\nconst name = input(\"What's your name? \")\n```",

        // Standard library modules
        "math" => "**math** -- Math standard library module\n\nProvides mathematical functions and constants.\n```ni\nimport math\nprint(math.sqrt(16))  // 4.0\nprint(math.PI)        // 3.14159...\n```\n\n**Functions:** abs, min, max, clamp, sqrt, floor, ceil, round, sin, cos, tan, asin, acos, atan, atan2, pow, lerp\n\n**Constants:** PI, TAU, INF",
        "random" => "**random** -- Random number module\n\nProvides random number generation.\n```ni\nimport random\nrandom.seed(42)\nvar roll = random.int(1, 6)\n```\n\n**Functions:** int, float, bool, chance, choice, shuffle, seed",
        _ => return None,
    };

    Some(Hover {
        contents: HoverContents::Markup(MarkupContent {
            kind: MarkupKind::Markdown,
            value: content.to_string(),
        }),
        range: None,
    })
}
