local unit = Unit
assert(unit ~= nil)

local test = Test({ name = "ok" })
assert(test.name == "ok")

local opt = OptionalString.Some("ok")
assert(opt.some[1] == "ok")

local opt = OptionalString.None
assert(opt.none == true)
