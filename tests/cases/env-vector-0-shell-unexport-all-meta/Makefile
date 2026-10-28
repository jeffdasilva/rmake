$(shell cc -O2 -o capture capture.c)
unexport MAKEFLAGS MFLAGS MAKELEVEL SHELL
$(info vector=[$(shell ./capture names)])
all: ; @true
