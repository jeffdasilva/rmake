all: a | b .WAIT c;@echo "$^:$|"
a b:;@touch $@
c:;@test -f a && test -f b;touch c
