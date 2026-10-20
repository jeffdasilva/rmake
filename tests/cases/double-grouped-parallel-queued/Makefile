all: left right
left: a
right: b
a b &:: input;@printf group >> count;touch -d @1000000000 a b
input:;@sleep .03;touch input
all:;@test "$$(cat count)" = group
