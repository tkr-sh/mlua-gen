local test = Test({ name = "name", int = 32 })
assert(test.new().name == "new")
assert(test.name == "name")
assert(test.int == 32)
assert(test:uwu(3, 2) == 37)
test:owo()
assert(test.int == 42)
