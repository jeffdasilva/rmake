$(file >seen,anchor)
unexport HOME READY RESULT
homedir = hdir
HOME = $(file >>seen,HOME)$(call homedir)
RESULT := missing
MAKEFLAGS += -I~/inc
-include answer.mk
$(info RESULT=[$(RESULT)])
.PHONY: all
all:;
