// Module with closures and higher-order functions

fun make_adder(n):
    fun adder(x):
        return x + n
    return adder

fun apply_twice(f, x):
    return f(f(x))

fun make_counter():
    var count = 0
    fun next():
        count = count + 1
        return count
    return next
