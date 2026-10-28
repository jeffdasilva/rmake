$(shell cc -O2 -o capture capture.c)
N:=A=B
export $(N)=strange
export A=ordinary
all: ; @./capture names
