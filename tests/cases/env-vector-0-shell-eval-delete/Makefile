$(shell cc -O2 -o capture capture.c)
export B=$(eval unexport A)B
export A=A
$(info vector=[$(shell ./capture names)])
all: ; @true
