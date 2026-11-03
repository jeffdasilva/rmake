MAKEFLAGS = A:=$$(file\ >>seen,A)one --include-dir=~/inc HOME=$$(MISSING_HOME)$$(file\ >>seen,SET)hdir B:=$$(file\ >>seen,B)two
-include answer.mk
$(info RESULT=[$(RESULT)] DIRS=[$(filter old/inc hdir/inc,$(.INCLUDE_DIRS))])
