local test = Test({ name = "hey" })
assert(test.name == "hey")
local testint = TestInt({ name = 1337 })
assert(testint.name == 1337)
