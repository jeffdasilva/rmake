$(shell cc -O2 -o capture capture.c)
export B=$(eval A:=changed)B
export A=old
$(info vector=[$(shell ./capture names)])
all: ; @true
