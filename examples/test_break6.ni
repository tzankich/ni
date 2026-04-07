fun make_adder(n):
    fun adder(x):
        return x + n
    return adder

var add5 := make_adder(5)

var found := 0
for i in 0..10:
    if i == 7:
        found = i
        break
print(found)
