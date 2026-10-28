$(shell cc -O2 -o capture capture.c)
export Z=$(eval export)Z
A=one
B=two
.ONESHELL:
all:
	@./capture names
	./capture A Z SHELL MAKELEVEL
