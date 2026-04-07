# Classes

## Defining a Class

A class bundles data (fields) with behavior (methods):

```ni
class Dog:
    var name = ""
    var breed = ""
    var energy = 100

    fun init(name, breed):
        self.name = name
        self.breed = breed
        self.energy = 100

    fun bark():
        print(`{self.name} says: Woof!`)
        self.energy -= 10

    fun is_tired():
        return self.energy < 20
```

Key points:
- Fields are declared with `var field = default` at the class level
- `init` is the constructor -- called automatically when you create an instance
- `self` refers to the current instance inside methods

## Creating Instances

Call the class name like a function:

```ni
var rex = Dog("Rex", "Shepherd")
var spot = Dog("Spot", "Dalmatian")

rex.bark()             // Rex says: Woof!
print(rex.energy)      // 90
print(spot.energy)     // 100 (separate instance)
```

Each instance has its own copy of the fields.

## Accessing Fields and Methods

Use dot notation:

```ni
print(rex.name)        // Rex
rex.energy = 50        // set a field directly
rex.bark()             // call a method
```

## The init Method

`init` runs automatically when an instance is created:

```ni
class Player:
    var name = ""
    var hp = 0
    var inventory = []

    fun init(name, hp = 100):
        self.name = name
        self.hp = hp
        self.inventory = []    // each player gets their own list

var hero = Player("Arthur")        // hp defaults to 100
var boss = Player("Dragon", 500)   // hp is 500
```

Always initialize list and map fields inside `init` -- otherwise all instances share the same list from the class default.

## Methods Returning Values

Methods work just like functions:

```ni
class Circle:
    var radius = 0

    fun init(radius):
        self.radius = radius

    fun area():
        return 3.14159 * self.radius * self.radius

    fun circumference():
        return 2 * 3.14159 * self.radius

var c = Circle(5)
print(c.area())             // 78.53975
print(c.circumference())    // 31.4159
```

## Inheritance

A class can extend another class with `extends`:

```ni
class Animal:
    var name = ""
    var sound = ""

    fun init(name, sound):
        self.name = name
        self.sound = sound

    fun speak():
        print(`{self.name} says: {self.sound}`)

class Cat extends Animal:
    var indoor = true

    fun init(name, indoor = true):
        super.init(name, "Meow")
        self.indoor = indoor

    fun describe():
        var location = "indoor" if self.indoor else "outdoor"
        print(`{self.name} is an {location} cat`)
```

- `extends` sets up the parent class
- `super.init(...)` calls the parent's constructor
- `super.method_name()` calls any parent method
- Child classes can override parent methods

```ni
var whiskers = Cat("Whiskers")
whiskers.speak()       // Whiskers says: Meow (inherited method)
whiskers.describe()    // Whiskers is an indoor cat
```

## A Practical Example

Here's a class hierarchy from the Holy Grail quest example:

```ni
class Knight:
    var name = ""
    var hp = 100

    fun init(name):
        self.name = name
        self.hp = 100

    fun is_alive():
        return self.hp > 0

    fun take_damage(amount):
        self.hp = self.hp - amount
        if self.hp < 0:
            self.hp = 0

class BlackKnight:
    var limbs = 4

    fun init():
        self.limbs = 4

    fun lose_limb():
        if self.limbs > 0:
            self.limbs = self.limbs - 1

    fun can_fight():
        return self.limbs > 0

spec "knight survives fight":
    var knight = Knight("Arthur")
    var bk = BlackKnight()
    while bk.can_fight() and knight.is_alive():
        bk.lose_limb()
        if bk.can_fight():
            knight.take_damage(15)
    assert knight.is_alive()
    assert not bk.can_fight()
```

## Enums

Enums define a fixed set of named values:

```ni
enum Direction:
    north
    south
    east
    west

var facing = Direction.north

match facing:
    when Direction.north:
        print("Going north")
    when Direction.south:
        print("Going south")
```

Enums can have explicit values:

```ni
enum Priority:
    low = 1
    medium = 2
    high = 3
```

## What's Next

You can build structured data with classes. Let's learn about [error handling](07-error-handling.md) -- dealing with things that go wrong.
