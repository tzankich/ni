// Classes in Ni
class Animal:
    fun init(name, sound):
        self.name = name
        self.sound = sound

    fun speak():
        print(self.name + " says " + self.sound)

class Dog extends Animal:
    fun init(name):
        super.init(name, "Woof!")

    fun fetch():
        print(self.name + " fetches the ball!")

var dog := Dog("Rex")
dog.speak()
dog.fetch()

var cat := Animal("Whiskers", "Meow!")
cat.speak()
