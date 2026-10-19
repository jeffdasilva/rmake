all: a b .WAIT c d
a b:;@sleep .01;touch $@
c d:;@test -f a && test -f b;touch $@
