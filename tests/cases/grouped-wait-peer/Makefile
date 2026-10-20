all: a b other
a b &:;@touch a b
b: first .WAIT second
first:;@sleep .04;touch first
second:;@test -f first && touch second
other:;@touch other
