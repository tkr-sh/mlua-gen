-- Active tuple variant — proxy with 1-based indexing.
assert(animal.dog[1] == "rex")
assert(animal.dog[2] == 3)

-- In-place writes propagate.
animal.dog[1] = "doggy"
animal.dog[2] = 7
assert(animal.dog[1] == "doggy")
assert(animal.dog[2] == 7)

-- Inactive variant returns nil.
assert(animal.pig == nil)

-- Whole-variant replacement still works.
animal.dog = { "rex2", 9 }
assert(animal.dog[1] == "rex2")
assert(animal.dog[2] == 9)
