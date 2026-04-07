// coffee.ni -- A small coffee shop modeled in Ni

enum Size:
    small = 1
    medium = 2
    large = 3

fun size_label(size):
    match size:
        when 1: return "small"
        when 2: return "medium"
        when 3: return "large"
        when _: fail "unknown size"

class Drink:
    var name := ""
    var size := Size.small
    var extras := []

    fun init(name, size):
        self.name = name
        self.size = size
        self.extras = []

    fun add_extra(name):
        self.extras = self.extras + [name]

    fun base_price():
        if self.size == Size.small:  return 3.00
        if self.size == Size.medium: return 4.00
        return 5.00

    fun total():
        var price := self.base_price()
        for extra in self.extras:
            price = price + 0.75
        return price

class Order:
    var drinks := []
    var discount_pct := 0

    fun init():
        self.drinks = []
        self.discount_pct = 0

    fun add(drink):
        self.drinks = self.drinks + [drink]

    fun subtotal():
        var sum := 0.0
        for d in self.drinks:
            sum = sum + d.total()
        return sum

    fun apply_discount(pct):
        self.discount_pct = pct

    fun total():
        var sub := self.subtotal()
        return sub - sub * self.discount_pct / 100

fun loyalty_discount(order):
    if order.drinks.length >= 3:
        order.apply_discount(10)


// ============================================================
// Specs -- "I'd like to have an argument, please."
// ============================================================

spec "Drink pricing":
    given "a medium latte":
        var latte := Drink("latte", Size.medium)

        then "base price is 4.00":
            assert latte.base_price() == 4.00

        // Each 'then' path re-runs from its root 'given'.
        // The latte below is a *fresh* medium latte -- no oat milk yet.
        when "adding oat milk":
            latte.add_extra("oat milk")

            then "one extra costs 0.75 more":
                assert latte.total() == 4.75

            // This nested 'when' runs after its parent -- the latte
            // already has oat milk, so vanilla is the second extra.
            when "also adding vanilla":
                latte.add_extra("vanilla")

                then "both extras are charged":
                    assert latte.total() == 5.50

spec "Order with discount":
    given "two drinks":
        var order := Order()
        order.add(Drink("espresso", Size.small))
        order.add(Drink("cappuccino", Size.large))

        then "subtotal is the sum":
            assert order.subtotal() == 8.00

        when "applying 20% off":
            order.apply_discount(20)

            then "total reflects the discount":
                assert order.total() == 6.40

            // This 'then' shares the 'when' above, but runs
            // its own isolated copy -- the discount was applied,
            // yet subtotal is unaffected.
            then "subtotal stays the same":
                assert order.subtotal() == 8.00

spec "Loyalty rewards":
    given "a three-drink order":
        var order := Order()
        order.add(Drink("drip", Size.small))
        order.add(Drink("mocha", Size.large))
        order.add(Drink("tea", Size.small))
        loyalty_discount(order)

        then "earns 10% off":
            // 3.00 + 5.00 + 3.00 = 11.00, minus 10% = 9.90
            assert order.total() == 9.90

// Data-driven: same spec, multiple rows.
// BUG: Large is 5.00, not 5.50 -- the third row will fail.
spec "Size pricing" each (
    ["size": Size.small,  "expect": 3.00],
    ["size": Size.medium, "expect": 4.00],
    ["size": Size.large,  "expect": 5.50],
):
    given "a drink of that size":
        var row := __row__
        then "base price matches":
            assert Drink("any", row["size"]).base_price() == row["expect"]

spec "Error handling":
    given "an invalid size value":
        result := try size_label(99)
        then "try catches the failure":
            assert result == none
