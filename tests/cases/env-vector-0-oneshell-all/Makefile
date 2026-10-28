$(shell cc -O2 -o capture capture.c)
.EXPORT_ALL_VARIABLES:
A=one
B=two
.ONESHELL:
all:
	@./capture names
	./capture A B C RAW SHELL MAKELEVEL
