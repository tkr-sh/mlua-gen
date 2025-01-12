local unit = Test.Unit
local tuple = Test.Tuple ( "test", 20 )
local named = Test.Named { name = "test", int = 20 }

assert(unit.unit)
assert(tuple.tuple[1] == "test")
assert(tuple.tuple[2] == 20)
assert(named.named.name == "test")
assert(named.named.int == 20)
