all: x.out
x.out: %.out: %.in .WAIT after;@echo "$<:$^:$*"
after:;@touch after
