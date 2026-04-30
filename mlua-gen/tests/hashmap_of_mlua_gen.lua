h.map["a"].x = 99
assert(h.map["a"].x == 99)
assert(peek_x("a") == 99)

h.map["b"] = Inner { x = 77 }
assert(h.map["b"].x == 77)
assert(peek_x("b") == 77)
