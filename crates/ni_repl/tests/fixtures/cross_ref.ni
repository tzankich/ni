// Module with cross-function references for import testing

fun helper(x):
    return x + 1

fun use_helper(x):
    return helper(x)

fun make_map(val):
    return ["key": val]

fun inner(a, meta):
    return a

fun outer(a):
    return inner(a, ["method": "GET"])

fun chain_a(x):
    return chain_b(x) + 1

fun chain_b(x):
    return x * 2
