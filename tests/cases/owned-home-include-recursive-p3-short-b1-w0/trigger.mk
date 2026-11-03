MAKEFLAGS = A:=$$(file\ >>seen,A)one HOME=$$(file\ >>seen,SET)hdirÿ B:=$$(file\ >>seen,B)two -I~/inc
-include answer.mk
$(info RESULT=[$(RESULT)] DIRS=[$(filter oldÿ/inc hdirÿ/inc,$(.INCLUDE_DIRS))])
