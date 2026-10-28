$(shell cc -O2 -o capture capture.c)
export A=global
a%: export A=pattern
$(info vector=[$(shell ./capture names)])
all: ; @true
