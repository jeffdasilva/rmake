$(shell cc -O2 -o capture capture.c)
A=file
export
unexport
$(info reader=[$(shell ./capture A)])
.SECONDEXPANSION:
all: $$(shell ./capture A > observed)
	@./capture A
