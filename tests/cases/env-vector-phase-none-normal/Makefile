$(shell cc -O2 -o capture capture.c)
A=file
$(info reader=[$(shell ./capture A)])
all: ; @./capture A
