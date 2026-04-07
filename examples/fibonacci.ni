// Fibonacci in Ni
fun fib(n):
    if n <= 1:
        return n
    return fib(n - 1) + fib(n - 2)

for i in 0..10:
    print(fib(i))
