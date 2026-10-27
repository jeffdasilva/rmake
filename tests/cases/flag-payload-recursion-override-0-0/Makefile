override MAKEFLAGS = X=flag
all: ; @$(info parent=[$(X)]/$(origin X))$(MAKE) --no-print-directory -f child.mk
