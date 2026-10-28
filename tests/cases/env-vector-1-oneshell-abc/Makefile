$(shell cc -O2 -o capture capture.c)
export A=one
export B=two
export C=three
.ONESHELL:
all:
	@./capture names
	./capture A B C RAW SHELL MAKELEVEL
