// Module with class inheritance for import testing

class Base:
    fun init(name):
        self.name = name

    fun greet():
        return "Hello, " + self.name

class Derived extends Base:
    fun init(name, title):
        super.init(name)
        self.title = title

    fun formal_greet():
        return self.title + " " + self.greet()
