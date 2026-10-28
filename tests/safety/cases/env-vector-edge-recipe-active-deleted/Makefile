$(shell cc -O2 -o capture capture.c)
export A=$(eval undefine A)$(shell ./capture A)
all: ; @./capture names
