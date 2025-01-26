local pig = Animal.Pig
local dog = Animal.Dog("doggo", 20)
local cat = Animal.Cat({ name = "nyan", age = 20 })

-- Accessing basic data 
---- Pig
assert(pig.pig)
---- Dog
assert(dog.dog[1] == "doggo")
assert(dog.dog[2] == 20)
---- Cat
assert(cat.cat.name == "nyan")
assert(cat.cat.age == 20)

-- Changing value
local new_dog = dog.dog
new_dog[1] = "doggy"
assert(dog.dog[1] == "doggo")
assert(new_dog[1] == "doggy")
dog.dog = new_dog

assert(dog.dog[1] == "doggy")
assert(dog.dog[2] == 20)


-- Nil when accessing wrong value
assert(pig.dog == nil)
assert(pig.cat == nil)

assert(dog.pig == nil)
assert(dog.cat == nil)

assert(cat.pig == nil)
assert(cat.dog == nil)

-- Note: If you try to do cat.Pig or cat.Dog or cat.Cat, it will error.
