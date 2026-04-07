// Module with class definitions for import testing

class Animal:
    fun init(name, sound):
        self.name = name
        self.sound = sound

    fun speak():
        return self.name + " says " + self.sound

class Counter:
    fun init():
        self.count = 0

    fun increment():
        self.count = self.count + 1
        return self.count

fun make_animal(name, sound):
    return Animal(name, sound)
