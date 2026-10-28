$(shell cc -O2 -o capture capture.c)
export A=one
undefine A
export A=two
all: ; @./capture names
