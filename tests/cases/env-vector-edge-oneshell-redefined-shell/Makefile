$(shell cc -O2 -o capture capture.c)
SHELL=/bin/sh
unexport SHELL
.ONESHELL:
all:
	@./capture names
	./capture A Z SHELL MAKELEVEL
