$(shell cc -O2 -o capture capture.c)
export A=one
undefine A
export A=two
.ONESHELL:
all:
	@./capture names
	./capture A B C RAW SHELL MAKELEVEL
