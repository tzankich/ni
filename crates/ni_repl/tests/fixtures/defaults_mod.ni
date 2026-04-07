// Module with functions using default parameters

fun greet(name, greeting = "Hello"):
    return greeting + ", " + name + "!"

fun add(a, b = 0, c = 0):
    return a + b + c
