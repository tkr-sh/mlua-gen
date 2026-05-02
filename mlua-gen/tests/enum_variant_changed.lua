local p = state.active
assert(p.x == 1)

state.other = { 42 }
assert(state.active == nil)
assert(state.other[1] == 42)

-- Subsequent reads/writes through the stale proxy must not silently
-- succeed: they error with "variant changed under proxy".
local ok, err = pcall(function() return p.x end)
assert(not ok, "expected read through stale proxy to error")
assert(string.find(tostring(err), "variant changed"), tostring(err))

local ok2, err2 = pcall(function() p.x = 99 end)
assert(not ok2, "expected write through stale proxy to error")
assert(string.find(tostring(err2), "variant changed"), tostring(err2))
