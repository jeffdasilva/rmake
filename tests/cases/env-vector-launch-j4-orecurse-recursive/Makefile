$(shell cc -O2 -o capture capture.c)
export A=one
export B=two
all: ; @$(MAKE) --no-print-directory -f child.mk child
