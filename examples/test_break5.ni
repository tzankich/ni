fun dummy(x):
    return x

class Foo:
    fun init():
        self.x = 1

var found := 0
for i in 0..10:
    if i == 7:
        found = i
        break
print(found)
