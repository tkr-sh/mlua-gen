local human = Human_.default()
human:set_age(42)
human.name = "Martin"
assert(human.name == "Martin")
assert(human:age() == "42 years old")
