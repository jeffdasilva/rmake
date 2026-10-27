export X
MAKEFLAGS += X:=flag
all: ; @$(info parent=[$(X)]/$(origin X))$(MAKE) -f child.mk
