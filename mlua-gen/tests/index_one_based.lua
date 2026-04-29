local h = Holder { xs = { 7, 8, 9 } }

assert(h.xs[1] == 7)
assert(h.xs[2] == 8)
assert(h.xs[3] == 9)

-- Reading at 0: clean error. Writing at 0: still panics via `IntoRustIndex`.
local ok = pcall(function() return h.xs[0] end)
assert(not ok)
local ok2 = pcall(function() h.xs[0] = 5 end)
assert(not ok2)
