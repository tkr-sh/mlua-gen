assert(a.b.c.v == 0)

a.b.c.v = 42
assert(a.b.c.v == 42)
assert(peek() == 42)

a.b.c.v = 7
assert(a.b.c.v == 7)
assert(peek() == 7)
