assert(hits() == 0)

-- Setter for an unnamed variant.
state.active = { 5 }
assert(hits() == 1)
assert(state.active[1] == 5)
assert(hits() == 1)

-- Setter for a named variant.
state.labelled = { name = "x", count = 1 }
assert(hits() == 2)
assert(state.labelled.name == "x")
assert(state.labelled.count == 1)
assert(hits() == 2)
