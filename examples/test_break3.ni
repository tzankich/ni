// Simulate the context before break test
var x := 10
var y := 20
var greeting := "Hello" + " " + "World"
var name := "" or "Default"
var val := 42 and 99
var count := 0
while count < 3:
    count = count + 1
var sum := 0
for i in 0..5:
    sum = sum + i
var items := [10, 20, 30]
items.add(40)
var stats := ["hp": 100, "mp": 50]

fun add(a, b):
    return a + b

fun greet2(name2, prefix = "Hello"):
    return prefix + ", " + name2 + "!"

fun make_adder(n):
    fun adder(x2):
        return x2 + n
    return adder

var add5 := make_adder(5)

class Point:
    fun init(x3, y3):
        self.x = x3
        self.y = y3

var p := Point(3, 4)

class Point3D extends Point:
    fun init(x3, y3, z3):
        super.init(x3, y3)
        self.z = z3

var p3 := Point3D(1, 2, 3)

enum Color:
    red = 0
    green = 1
    blue = 2

var score := 85

// NOW the break test
var found := 0
for i in 0..10:
    if i == 7:
        found = i
        break
print(found)
