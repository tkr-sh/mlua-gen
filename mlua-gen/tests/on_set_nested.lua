assert(hits() == 0)

-- Depth-2 write fires once.
r.mid.leaf.v = 1
assert(hits() == 1)
assert(peek_leaf() == 1)
assert(r.mid.leaf.v == 1)

-- Collection element field write fires once.
r.items[1].v = 99
assert(hits() == 2)
assert(peek_item(1) == 99)
assert(r.items[1].v == 99)

-- Whole-element replace via collection __newindex fires once.
r.items[2] = Leaf { v = 77 }
assert(hits() == 3)
assert(peek_item(2) == 77)
assert(r.items[2].v == 77)
