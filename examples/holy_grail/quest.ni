// quest.ni -- Holy Grail quest logic and tests
// "We are the Knights Who Say... Ni!"

class Knight:
    var name := ""
    var hp := 100
    var limbs := 4
    var inventory := []
    var quest_complete := false

    fun init(name):
        self.name = name
        self.hp = 100
        self.limbs = 4
        self.inventory = []
        self.quest_complete = false

    fun is_alive():
        return self.hp > 0

    fun has_item(item):
        for i in self.inventory:
            if i == item:
                return true
        return false

    fun add_item(item):
        self.inventory = self.inventory + [item]

    fun take_damage(amount):
        self.hp = self.hp - amount
        if self.hp < 0:
            self.hp = 0

// The Black Knight -- loses limbs but insists on fighting
class BlackKnight:
    var limbs := 4
    var taunts := []

    fun init():
        self.limbs = 4
        self.taunts = ["Tis but a scratch!", "It's just a flesh wound!", "I'm invincible!", "Oh, had enough, eh?"]

    fun lose_limb():
        if self.limbs > 0:
            self.limbs = self.limbs - 1
        return self.taunt()

    fun taunt():
        var index := 3 - self.limbs
        if index < 0:
            index = 0
        if index > 3:
            index = 3
        return self.taunts[index]

    fun can_fight():
        return self.limbs > 0

    fun damage():
        if self.limbs >= 3:
            return 25
        if self.limbs == 2:
            return 15
        if self.limbs == 1:
            return 5
        return 0

// The Holy Hand Grenade of Antioch
// "Count to three, no more, no less."
fun throw_holy_hand_grenade(count):
    if count == 3:
        return "boom"
    if count == 5:
        return "three_sir"
    if count < 3:
        return "not_yet"
    return "wrong"

// The Bridge of Death
class Bridgekeeper:
    var questions_asked := 0

    fun init():
        self.questions_asked = 0

    fun ask(answer, correct_answer):
        self.questions_asked = self.questions_asked + 1
        if answer == correct_answer:
            return "pass"
        return "death"

    fun ask_swallow_speed(answer):
        self.questions_asked = self.questions_asked + 1
        if answer == "what_kind":
            return "bridgekeeper_dies"
        return "death"

    fun crossed():
        return self.questions_asked >= 3

// The Knights Who Say Ni -- demand shrubberies
class KnightsWhoSayNi:
    var satisfied := false
    var shrubberies := 0
    var has_herring := false

    fun init():
        self.satisfied = false
        self.shrubberies = 0
        self.has_herring = false

    fun demand():
        if self.shrubberies == 0:
            return "shrubbery"
        if self.shrubberies == 1 and not self.has_herring:
            return "another_and_herring"
        return "ni"

    fun offer_shrubbery():
        self.shrubberies = self.shrubberies + 1
        if self.shrubberies == 1:
            return "need_another"
        if self.shrubberies >= 2 and self.has_herring:
            self.satisfied = true
            return "you_may_pass"
        return "need_herring"

    fun offer_herring():
        self.has_herring = true
        if self.shrubberies >= 2:
            self.satisfied = true
            return "you_may_pass"
        return "need_shrubbery"

// Combat round between a knight and the Black Knight
fun combat_round(knight, black_knight):
    var taunt := black_knight.lose_limb()
    if black_knight.can_fight():
        var dmg := black_knight.damage()
        knight.take_damage(dmg)
    return taunt

// Full quest sequence -- returns true if grail found
fun attempt_quest(knight):
    // Phase 1: Get past the Knights Who Say Ni
    var ni_knights := KnightsWhoSayNi()
    knight.add_item("shrubbery")
    ni_knights.offer_shrubbery()
    knight.add_item("shrubbery")
    ni_knights.offer_shrubbery()
    knight.add_item("herring")
    ni_knights.offer_herring()

    if not ni_knights.satisfied:
        return false

    // Phase 2: Defeat the Black Knight
    var bk := BlackKnight()
    while bk.can_fight() and knight.is_alive():
        combat_round(knight, bk)

    if not knight.is_alive():
        return false

    // Phase 3: Cross the Bridge of Death
    var bridge := Bridgekeeper()
    bridge.ask("Lancelot", "Lancelot")
    bridge.ask("grail", "grail")
    bridge.ask_swallow_speed("what_kind")

    // Phase 4: Holy Hand Grenade vs the Killer Rabbit
    var result := throw_holy_hand_grenade(3)

    knight.quest_complete = true
    return true


// ============================================================
// SPECS -- "And now for something completely different."
// ============================================================

// --- The Black Knight ---

spec "The Black Knight":
    given "a fresh Black Knight":
        var bk := BlackKnight()
        then "starts with 4 limbs and can fight":
            assert bk.limbs == 4
            assert bk.can_fight()
        when "losing limbs in combat":
            bk.lose_limb()
            bk.lose_limb()
            bk.lose_limb()
            bk.lose_limb()
            then "loses them one at a time with correct taunts":
                assert bk.limbs == 0
            then "cannot fight with zero limbs":
                assert not bk.can_fight()
            then "limbs floor at zero after extra hits":
                bk.lose_limb()
                bk.lose_limb()
                assert bk.limbs == 0
        when "taking damage":
            then "damage decreases as limbs are lost":
                assert bk.damage() == 25
                bk.lose_limb()
                assert bk.damage() == 25
                bk.lose_limb()
                assert bk.damage() == 15
                bk.lose_limb()
                assert bk.damage() == 5
                bk.lose_limb()
                assert bk.damage() == 0

// --- The Holy Hand Grenade ---

spec "The Holy Hand Grenade" each (
    ["count": 1, "expected": "not_yet"],
    ["count": 2, "expected": "not_yet"],
    ["count": 3, "expected": "boom"],
    ["count": 4, "expected": "wrong"],
    ["count": 5, "expected": "three_sir"],
):
    given "a count":
        var row := __row__
        then "result matches expected":
            assert throw_holy_hand_grenade(row["count"]) == row["expected"]

// --- The Bridge of Death ---

spec "The Bridge of Death":
    given "the Bridgekeeper":
        var bridge := Bridgekeeper()
        when "answering correctly":
            then "you pass":
                assert bridge.ask("Arthur", "Arthur") == "pass"
        when "answering incorrectly":
            then "death":
                assert bridge.ask("I dunno", "Lancelot") == "death"
        when "asked about swallow speed":
            then "asking back kills the bridgekeeper":
                assert bridge.ask_swallow_speed("what_kind") == "bridgekeeper_dies"
            then "guessing means death":
                assert bridge.ask_swallow_speed("11mph") == "death"
        when "all three questions answered":
            bridge.ask("Arthur", "Arthur")
            bridge.ask("grail", "grail")
            bridge.ask_swallow_speed("what_kind")
            then "bridge is crossed":
                assert bridge.crossed()

// --- Knights Who Say Ni ---

spec "The Knights Who Say Ni":
    given "the Knights Who Say Ni":
        var knights := KnightsWhoSayNi()
        then "demand a shrubbery first":
            assert knights.demand() == "shrubbery"
        when "offered one shrubbery":
            knights.offer_shrubbery()
            then "still not satisfied":
                assert not knights.satisfied
        when "offered two shrubberies and a herring":
            knights.offer_shrubbery()
            knights.offer_shrubbery()
            knights.offer_herring()
            then "satisfied":
                assert knights.satisfied
        when "offered herring before second shrubbery":
            knights.offer_shrubbery()
            knights.offer_herring()
            then "not satisfied until second shrubbery":
                assert not knights.satisfied
                knights.offer_shrubbery()
                assert knights.satisfied

// --- Combat ---

spec "Combat":
    given "Arthur facing the Black Knight":
        var knight := Knight("Arthur")
        var bk := BlackKnight()
        when "a single combat round":
            combat_round(knight, bk)
            then "Arthur takes damage and BK loses a limb":
                assert knight.hp < 100
                assert bk.limbs == 3
        when "fighting to the finish":
            while bk.can_fight() and knight.is_alive():
                combat_round(knight, bk)
            then "Arthur survives":
                assert not bk.can_fight()
                assert knight.is_alive()
    given "a Knight":
        then "HP floors at zero on massive damage":
            var robin := Knight("Robin")
            robin.take_damage(999)
            assert robin.hp == 0
        then "inventory tracks items":
            var k := Knight("Arthur")
            assert not k.has_item("shrubbery")
            k.add_item("shrubbery")
            assert k.has_item("shrubbery")

// --- Full Quest ---

spec "The Quest for the Holy Grail":
    given "Sir Arthur on a quest":
        var arthur := Knight("Arthur")
        var result := attempt_quest(arthur)
        then "completes the quest":
            assert result
            assert arthur.quest_complete
        then "survives the quest":
            assert arthur.is_alive()
        then "collects items during the quest":
            assert arthur.has_item("shrubbery")
            assert arthur.has_item("herring")
