$(shell cc -O2 -o capture capture.c)
export B=$(eval unexport A)B
export A=A
.ONESHELL:
all:
	@./capture names
	./capture A B C RAW SHELL MAKELEVEL
