$(file >seen,anchor)
unexport HOME Z RESULT
HOME = $(file >>seen,HOME)hdir
RESULT := missing
MAKEFLAGS = -I~/inc -- --include-dir=~/MISSING
-include answer.mk
$(info RESULT=[$(RESULT)] Z=[$(Z)])
.PHONY: all
all:;
