all: a b other
a b &:: first .WAIT second;@touch -d @1000000000 a b
first:;@sleep .04;touch first
second:;@test -f first && touch second
other:;@touch other
