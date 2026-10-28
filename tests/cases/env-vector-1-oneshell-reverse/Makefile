$(shell cc -O2 -o capture capture.c)
export Z=one
export Y=two
export X=three
.ONESHELL:
all:
	@./capture names
	./capture A B C RAW SHELL MAKELEVEL
