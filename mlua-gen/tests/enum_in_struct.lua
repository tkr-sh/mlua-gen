-- Read through the struct, then through the enum router proxy, then
-- through the active variant proxy.
assert(o.inner.active.x == 1)

-- Inactive variant returns nil.
assert(o.inner.idle == nil)
assert(o.inner.tup == nil)

-- In-place mutation, two levels deep.
o.inner.active.x = 42
assert(o.inner.active.x == 42)

-- Switching variant by writing through the router proxy.
o.inner.tup = { 7, 8 }
assert(o.inner.active == nil)
assert(o.inner.tup[1] == 7)
assert(o.inner.tup[2] == 8)

-- Mutate the new variant in place.
o.inner.tup[1] = 70
assert(o.inner.tup[1] == 70)
