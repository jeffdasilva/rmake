$(file >seen,anchor)
unexport HOME RESULT
HOME = $(file >>seen,HOME)hdir
RESULT := missing
MAKEFLAGS = --old-file=~/MISSING -I~/inc
-include answer.mk
$(info RESULT=[$(RESULT)])
.PHONY: all
all:;
