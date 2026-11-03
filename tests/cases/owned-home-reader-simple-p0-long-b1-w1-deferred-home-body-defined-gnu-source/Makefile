$(file >seen,anchor)
unexport HOME A B RESULT
HOME = $(MISSING_HOME)$(file >>seen,OLD)oldÿ
RESULT := missing
MAKEFLAGS = --include-dir=~/inc A:=$$(file\ >>seen,A)one HOME:=$$(file\ >>seen,SET)hdirÿ B:=$$(file\ >>seen,B)two
-include answer.mk
$(info RESULT=[$(RESULT)] DIRS=[$(filter oldÿ/inc hdirÿ/inc,$(.INCLUDE_DIRS))])
.PHONY: all
all:;
