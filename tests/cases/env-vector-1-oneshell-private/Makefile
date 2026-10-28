$(shell cc -O2 -o capture capture.c)
export X=global
%.out: private X=pattern
.ONESHELL:
all:
	@./capture names
	./capture A B C RAW SHELL MAKELEVEL
