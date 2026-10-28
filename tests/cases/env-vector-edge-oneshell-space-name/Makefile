$(shell cc -O2 -o capture capture.c)
N:=A B
export $(N)=spaces
.ONESHELL:
all:
	@./capture names
	./capture A Z SHELL MAKELEVEL
