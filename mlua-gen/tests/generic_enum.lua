local str = Optional.Some("String")
local none = Optional.None
local int = OptionalInt.Some(10)

local some = int.some
some[1] = 100
int.some = some

assert(int.some[1] == 100)
assert(none.none)
assert(str.some[1] == "String")
