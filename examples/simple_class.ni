class Foo:
    fun init(x):
        self.x = x

    fun get_x():
        return self.x

var f := Foo(42)
print(f.get_x())
