$(shell cc -O2 -o capture capture.c)
export X=global
%.out: private X=pattern
all: ; @./capture names
