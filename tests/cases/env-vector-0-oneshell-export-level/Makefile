$(shell cc -O2 -o capture capture.c)
export MAKELEVEL=$(info expanded)file
.ONESHELL:
all:
	@./capture names
	./capture A B C RAW SHELL MAKELEVEL
