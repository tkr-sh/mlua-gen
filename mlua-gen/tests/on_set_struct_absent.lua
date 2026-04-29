local plain = Plain({ value = 0 })

assert(sentinel() == 0)

plain.value = 42
assert(plain.value == 42)

-- The sentinel must still be 0: no `on_set` was configured, so the macro
-- must not have wired anything up.
assert(sentinel() == 0)
