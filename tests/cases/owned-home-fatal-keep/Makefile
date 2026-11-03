$(file >seen,anchor)
unexport HOME
HOME = $(file >>seen,HOME)$(error home-fatal)
MAKEFLAGS += -I~/inc
$(file >>seen,FORBIDDEN)
.PHONY: all
all:;
