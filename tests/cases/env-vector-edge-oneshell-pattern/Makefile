$(shell cc -O2 -o capture capture.c)
export A=global
a%: export A=pattern
.ONESHELL:
all:
	@./capture names
	./capture A Z SHELL MAKELEVEL
