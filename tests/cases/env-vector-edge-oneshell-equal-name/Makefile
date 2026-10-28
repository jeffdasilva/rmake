$(shell cc -O2 -o capture capture.c)
N:=A=B
export $(N)=strange
export A=ordinary
.ONESHELL:
all:
	@./capture names
	./capture A Z SHELL MAKELEVEL
