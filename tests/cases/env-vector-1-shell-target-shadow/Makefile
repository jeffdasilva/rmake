$(shell cc -O2 -o capture capture.c)
export A=global
all: A=local
$(info vector=[$(shell ./capture names)])
all: ; @true
