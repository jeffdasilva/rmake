$(file >seen,anchor)
unexport HOME RESULT
HOME = $(file >>seen,HOME)
RESULT := missing
MAKEFLAGS += -I~/inc
-include answer.mk
$(info RESULT=[$(RESULT)])
.PHONY: all
all:;
