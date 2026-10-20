all: left right
left: a
right: b
a b &: input;@printf group >> count;touch a b
input:;@sleep .03;touch input
all:;@test "$$(cat count)" = group
