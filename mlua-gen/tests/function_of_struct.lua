local human = Human.default()
assert(human.i1 ~= nil)
assert(human.i1 == "")
human.i1 = "heart"
assert(human.i1 == "heart")
local martin = Human.new_martin()
local martin2 = Human("Martin")
assert(martin.i1 == martin2.i1)
