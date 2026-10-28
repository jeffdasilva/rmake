$(shell cc -O2 -o capture capture.c)
export PATH=.:/usr/bin:/bin
.ONESHELL:
all:
	@./capture names
	./capture A B C RAW SHELL MAKELEVEL
