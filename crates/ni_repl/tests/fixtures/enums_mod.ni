// Module with enum definitions for import testing

enum Color:
    red = 0
    green = 1
    blue = 2

fun color_name(c):
    match c:
        when Color.red:
            return "red"
        when Color.green:
            return "green"
        when Color.blue:
            return "blue"
