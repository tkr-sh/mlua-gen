local human = Human({ age = 43, name = "Eden" })
human.age = 42
human.name = "Martin"
assert(human.name_age == "Martin (42)")
assert(human:age_in_next_years(3) == 45)
