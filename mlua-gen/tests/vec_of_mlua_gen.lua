assert(h.items[1].x == 1)
assert(h.items[2].x == 2)

h.items[2].x = 99
assert(peek_x(2) == 99)

-- Whole-element replacement.
h.items[1] = Inner { x = 7 }
assert(peek_x(1) == 7)
