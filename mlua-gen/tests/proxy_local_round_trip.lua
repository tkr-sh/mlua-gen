local proxy = o.inner
proxy.x = 42
assert(proxy.x == 42)
assert(peek() == 42)
