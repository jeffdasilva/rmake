$(shell cc -O2 -o capture capture.c)
export A=global
a%: private A=pattern
all: ; @./capture names
