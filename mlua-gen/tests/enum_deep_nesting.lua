-- Path: a.b.mid.active.leaf.v — that's 5 proxy hops before the leaf:
--   1. `a.b`           → struct-field proxy on A     (path = [Field b])
--   2. `.mid`          → struct-field proxy on B     (path = [..., Field mid])
--   3. `.active`       → enum router proxy on Mid    (path = [..., Variant active])
--   4. `.leaf`         → variant-fields proxy        (path = [..., Field leaf])
--   5. `.v = X`        → leaf write through 5 levels
assert(a.b.mid.active.leaf.v == 1)

a.b.mid.active.leaf.v = 99
assert(a.b.mid.active.leaf.v == 99)

-- Tuple variant: same depth. Switch via the variant accessor (sets the
-- whole `Pair` variant from a sequence table).
a.b.mid.pair = { { v = 10 }, { v = 20 } }
assert(a.b.mid.pair[1].v == 10)
assert(a.b.mid.pair[2].v == 20)

a.b.mid.pair[1].v = 111
a.b.mid.pair[2].v = 222
assert(a.b.mid.pair[1].v == 111)
assert(a.b.mid.pair[2].v == 222)

-- Inactive variant still reads as nil through the deep chain.
assert(a.b.mid.active == nil)
assert(a.b.mid.idle == nil)
