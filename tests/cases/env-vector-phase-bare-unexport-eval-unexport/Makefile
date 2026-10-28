$(shell cc -O2 -o capture capture.c)
A=file
export
unexport
$(info reader=[$(shell ./capture A)])
all: ; @$(eval unexport)./capture A
