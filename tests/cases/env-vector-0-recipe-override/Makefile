$(shell cc -O2 -o capture capture.c)
export A=one
override A=two
all: ; @./capture names
