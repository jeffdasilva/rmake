$(shell cc -O2 -o capture capture.c)
export A=global
all: private A=local
all: ; @./capture names
