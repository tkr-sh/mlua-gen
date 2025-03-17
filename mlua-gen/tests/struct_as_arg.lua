local qt = Quantity(42);

local test = Test({name = "apples"});

assert("42 apples" == test:display_qt_name(qt));
