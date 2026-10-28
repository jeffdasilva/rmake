$(shell cc -O2 -o capture capture.c)
all: export X=local
all: export Y=next
.ONESHELL:
all:
	@./capture names
	./capture A B C RAW SHELL MAKELEVEL
