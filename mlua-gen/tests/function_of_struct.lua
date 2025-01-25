local human = Human_.default()
assert(human.i1 ~= nil)
assert(human.i1 == "")
human.i1 = "heart"
assert(human.i1 == "heart")
local martin = Human_.new_martin()
local martin2 = Human("Martin")
assert(martin.i1 == martin2.i1)
