MAKEFLAGS = A:=$$(file\ >>seen,A)one HOME=$$(MISSING_HOME)$$(file\ >>seen,SET)hdir --include-dir=~/inc B:=$$(file\ >>seen,B)two
-include answer.mk
$(info RESULT=[$(RESULT)] DIRS=[$(filter old/inc hdir/inc,$(.INCLUDE_DIRS))])
