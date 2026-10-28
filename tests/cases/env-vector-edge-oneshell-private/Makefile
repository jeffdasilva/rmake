$(shell cc -O2 -o capture capture.c)
export A=global
all: private A=local
.ONESHELL:
all:
	@./capture names
	./capture A Z SHELL MAKELEVEL
