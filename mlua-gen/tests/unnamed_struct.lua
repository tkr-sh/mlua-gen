local test = Unnamed("name", 32)
assert(test.i1 == "name")
assert(test.i2 == 32)
test.i2 = 42
assert(test.i2 == 42)
