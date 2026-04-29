local h = Holder { xs = { 1, 2, 3 }, ys = { 10, 20, 30 } }

assert(h.xs[1] == 1)
assert(h.ys[1] == 10)

h.ys[1] = 99
assert(h.ys[1] == 99)

-- `xs` not in `set`: write goes to the proxy table, doesn't propagate.
h.xs[1] = 999
assert(h.xs[1] == 1)
