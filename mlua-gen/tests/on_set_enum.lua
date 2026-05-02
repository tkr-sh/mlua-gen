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

-- In-place mutation through the variant proxy fires `on_set` once per
-- top-level `=` (Phase 3).
state.labelled.name = "y"
assert(hits() == 3)
assert(state.labelled.name == "y")
assert(hits() == 3)

-- Switching variant first, then in-place mutation.
state.active = { 10 }
assert(hits() == 4)
state.active[1] = 20
assert(hits() == 5)
assert(state.active[1] == 20)
