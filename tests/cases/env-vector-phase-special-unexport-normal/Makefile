$(shell cc -O2 -o capture capture.c)
A=file
.EXPORT_ALL_VARIABLES:
unexport
$(info reader=[$(shell ./capture A)])
all: ; @./capture A
