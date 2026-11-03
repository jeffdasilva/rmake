$(file >seen,anchor)
unexport HOME RESULT
HOME = $(file >>seen,HOME)hdir
RESULT := missing
MAKEFLAGS = -I~/inc -f~/MISSING
-include answer.mk
$(info RESULT=[$(RESULT)])
.PHONY: all
all:;
