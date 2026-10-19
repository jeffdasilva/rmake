all: .WAIT a b .WAIT .WAIT c d .WAIT .WAIT
a b:;@sleep .01;touch $@
c d:;@test -f a && test -f b;touch $@
