local outer = Outer { inner = Inner { x = 7 } }

assert(outer.inner.x == 7)

outer.inner.x = 42
assert(outer.inner.x == 42)
