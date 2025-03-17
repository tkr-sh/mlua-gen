local human = Human.default()
assert(human[1] ~= nil)
assert(human[1] == "")
human[1] = "heart"
assert(human[1] == "heart")
local martin = Human.new_martin()
local martin2 = Human("Martin")
assert(martin[1] == martin2[1])
