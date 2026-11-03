MAKEFLAGS = A:=$$(file\ >>seen,A)one HOME=$$(MISSING_HOME)$$(file\ >>seen,SET)hdirÿ -I~/inc B:=$$(file\ >>seen,B)two
-include answer.mk
$(info RESULT=[$(RESULT)] DIRS=[$(filter oldÿ/inc hdirÿ/inc,$(.INCLUDE_DIRS))])
