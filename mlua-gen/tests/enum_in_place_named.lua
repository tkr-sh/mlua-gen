-- Active variant fields are read through a live proxy.
assert(state.labelled.name == "init")
assert(state.labelled.count == 0)

-- In-place writes propagate to the underlying enum.
state.labelled.name = "x"
state.labelled.count = 5
assert(state.labelled.name == "x")
assert(state.labelled.count == 5)

-- Inactive variant is nil.
assert(state.idle == nil)

-- Whole-variant replacement still works (legacy table form).
state.labelled = { name = "y", count = 7 }
assert(state.labelled.name == "y")
assert(state.labelled.count == 7)
