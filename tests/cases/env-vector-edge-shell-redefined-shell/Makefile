$(shell cc -O2 -o capture capture.c)
SHELL=/bin/sh
unexport SHELL
$(info vector=[$(shell ./capture names)])
all: ; @true
