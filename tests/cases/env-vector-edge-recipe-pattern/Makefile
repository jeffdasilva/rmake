$(shell cc -O2 -o capture capture.c)
export A=global
a%: export A=pattern
all: ; @./capture names
