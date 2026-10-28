$(shell cc -O2 -o capture capture.c)
export A=one
override A=two
$(info vector=[$(shell ./capture names)])
all: ; @true
