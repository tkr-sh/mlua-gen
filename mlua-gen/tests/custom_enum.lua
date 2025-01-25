local animal = Animal.Dog("Doggo")
assert(animal.dog ~= nil)
assert(animal.horse == "No horse")
assert(animal:name() == "Doggo")
