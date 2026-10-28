$(shell cc -O2 -o capture capture.c)
A=file
export
$(info reader=[$(shell ./capture A)])
all: ; @$(eval export)./capture A
