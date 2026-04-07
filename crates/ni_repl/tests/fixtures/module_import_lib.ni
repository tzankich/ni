// Module with functions that exercise different patterns

// Bug 1: Cross-function reference
fun helper(x):
    return x + 1

fun use_helper(x):
    return helper(x)

// Bug 2: Map literal with string key
fun make_map(val):
    return ["key": val]

// Bug 3: Function calling another function with map arg
fun inner(a, meta):
    return a

fun outer(a):
    return inner(a, ["method": "GET"])
