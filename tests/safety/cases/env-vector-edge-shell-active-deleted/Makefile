$(shell cc -O2 -o capture capture.c)
export A=$(eval undefine A)$(shell ./capture A)
$(info vector=[$(shell ./capture names)])
all: ; @true
