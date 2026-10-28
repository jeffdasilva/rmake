$(shell cc -O2 -o capture capture.c)
all: export X=local
all: export Y=next
$(info vector=[$(shell ./capture names)])
all: ; @true
