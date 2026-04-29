local p = Pair(Inner { x = 1 }, { 10, 20, 30 })

assert(p[1].x == 1)
p[1].x = 99
assert(p[1].x == 99)

assert(p[2][1] == 10)
assert(p[2][2] == 20)
p[2][1] = 11
assert(p[2][1] == 11)
