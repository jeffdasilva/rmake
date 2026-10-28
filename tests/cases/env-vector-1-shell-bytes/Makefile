$(shell cc -O2 -o capture capture.c)
export A=one
export RAW:=$(file <raw.bin)
$(info vector=[$(shell ./capture names)])
all: ; @true
