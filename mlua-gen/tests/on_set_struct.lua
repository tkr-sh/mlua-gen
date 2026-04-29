local app = App({ focused_buffer = 0, title = "init" })

-- No Lua-side write yet -> hook must not have fired.
assert(hits() == 0)

app.focused_buffer = 7
assert(hits() == 1)
assert(app.focused_buffer == 7)

app.title = "hello"
assert(hits() == 2)
assert(app.title == "hello")
assert(hits() == 2)

-- Sanity: writing the same value again still fires the hook (the contract is
-- "any setter call", not "any state change").
app.focused_buffer = 7
assert(hits() == 3)
