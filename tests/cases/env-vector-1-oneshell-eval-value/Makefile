$(shell cc -O2 -o capture capture.c)
export B=$(eval A:=changed)B
export A=old
.ONESHELL:
all:
	@./capture names
	./capture A B C RAW SHELL MAKELEVEL
