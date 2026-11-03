$(file >seen,anchor)
unexport HOME A B RESULT
HOME = $(MISSING_HOME)$(file >>seen,OLD)old
RESULT := missing
MAKEFLAGS = A:=$$(file\ >>seen,A)one HOME=$$(MISSING_HOME)$$(file\ >>seen,SET)hdir B:=$$(file\ >>seen,B)two -I~/inc
-include answer.mk
$(info RESULT=[$(RESULT)] DIRS=[$(filter old/inc hdir/inc,$(.INCLUDE_DIRS))])
.PHONY: all
all:;
