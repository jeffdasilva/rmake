$(shell cc -O2 -o capture capture.c)
unexport MAKELEVEL
.ONESHELL:
all:
	@./capture names
	./capture A B C RAW SHELL MAKELEVEL
