$(file >seen,anchor)
unexport HOME A B RESULT
HOME = $(file >>seen,OLD)oldÿ
RESULT := missing
MAKEFLAGS = A:=$$(file\ >>seen,A)one HOME:=$$(file\ >>seen,SET)hdirÿ B:=$$(file\ >>seen,B)two --include-dir=~/inc
-include answer.mk
$(info RESULT=[$(RESULT)] DIRS=[$(filter oldÿ/inc hdirÿ/inc,$(.INCLUDE_DIRS))])
.PHONY: all
all:;
