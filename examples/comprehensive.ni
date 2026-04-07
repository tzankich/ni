// Comprehensive Ni test

// === Arithmetic ===
print(5 + 3)
print(5 - 3)
print(5 * 3)
print(5 / 3)
print(5 % 3)
print(5.0 / 3)
print(-42)

// === Variables ===
var x := 10
var y := 20
print(x + y)

PI := 3
print(PI)

// === Strings ===
var greeting := "Hello" + " " + "World"
print(greeting)

// === Boolean and logic ===
print(true and false)
print(true or false)
print(not false)
print(1 == 1)
print(1 != 2)

// === and/or return determining value ===
var name := "" or "Default"
print(name)
var val := 42 and 99
print(val)

// === Control flow ===
if x > 5:
    print("x is big")
elif x > 0:
    print("x is positive")
else:
    print("x is zero or negative")

// === While loop ===
var count := 0
while count < 3:
    count = count + 1
print(count)

// === For loop ===
var sum := 0
for i in 0..5:
    sum = sum + i
print(sum)

// === Lists ===
var items := [10, 20, 30]
print(items[0])
print(items[-1])
print(items.length)

items.add(40)
print(items.length)

// === Maps ===
var stats := ["hp": 100, "mp": 50]
print(stats["hp"])

// === Functions ===
fun add(a, b):
    return a + b

print(add(3, 4))

fun greet(name, prefix = "Hello"):
    return prefix + ", " + name + "!"

print(greet("World"))

// === Nested functions / closures ===
fun make_adder(n):
    fun adder(x):
        return x + n
    return adder

var add5 := make_adder(5)
print(add5(10))

// === Classes ===
class Point:
    fun init(x, y):
        self.x = x
        self.y = y

    fun to_string():
        return "(" + to_string(self.x) + ", " + to_string(self.y) + ")"

var p := Point(3, 4)
print(p.x)
print(p.y)

// === Inheritance ===
class Point3D extends Point:
    fun init(x, y, z):
        super.init(x, y)
        self.z = z

var p3 := Point3D(1, 2, 3)
print(p3.x)
print(p3.z)

// === Enum ===
enum Color:
    red = 0
    green = 1
    blue = 2

print(Color.red)
print(Color.green)

// === Match ===
var score := 85
match score:
    case 100:
        print("Perfect!")
    case 0:
        print("Zero!")
    case _:
        print("Other score")

// === Break/Continue ===
var found := 0
for i in 0..10:
    if i == 7:
        found = i
        break
print(found)

// === Truthiness ===
print(not 0)
print(not "")
print(not none)
print(not 1)
print(not "hello")

// === Integer division ===
print(10 / 3)
print(10 / 5)
