local pig = Animal.Pig
local dog = Animal.Dog("doggo", 20)
local cat = Animal.Cat({ name = "nyan", age = 20 })

assert(pig.pig)
assert(dog.dog[1] == "doggo")
assert(dog.dog[2] == 20)
assert(cat.cat.name == "nyan")
assert(cat.cat.age == 20)

local new_dog = dog.dog
new_dog[1] = "doggy"
assert(dog.dog[1] == "doggo")
assert(new_dog[1] == "doggy")
dog.dog = new_dog

assert(dog.dog[1] == "doggy")
assert(dog.dog[2] == 20)
