$(file >seen,anchor)
unexport HOME A B RESULT
HOME = $(MISSING_HOME)$(file >>seen,OLD)old
RESULT := missing
MAKEFLAGS = A:=$$(file\ >>seen,A)one HOME:=$$(file\ >>seen,SET)hdir --include-dir=~/inc B:=$$(file\ >>seen,B)two
-include answer.mk
$(info RESULT=[$(RESULT)] DIRS=[$(filter old/inc hdir/inc,$(.INCLUDE_DIRS))])
.PHONY: all
all:;
