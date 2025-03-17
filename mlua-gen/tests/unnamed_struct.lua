local test = Unnamed("name", 32)
assert(test[1] == "name")
assert(test[2] == 32)
test[2] = 42
assert(test[2] == 42)
