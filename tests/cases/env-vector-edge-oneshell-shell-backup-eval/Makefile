$(shell cc -O2 -o capture capture.c)
export A=$(eval unexport SHELL)one
.ONESHELL:
all:
	@./capture names
	./capture A Z SHELL MAKELEVEL
