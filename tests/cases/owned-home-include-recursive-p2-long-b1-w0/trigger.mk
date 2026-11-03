MAKEFLAGS = A:=$$(file\ >>seen,A)one HOME=$$(file\ >>seen,SET)hdirÿ --include-dir=~/inc B:=$$(file\ >>seen,B)two
-include answer.mk
$(info RESULT=[$(RESULT)] DIRS=[$(filter oldÿ/inc hdirÿ/inc,$(.INCLUDE_DIRS))])
