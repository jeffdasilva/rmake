MAKEFLAGS = A:=$$(file\ >>seen,A)one HOME:=$$(file\ >>seen,SET)hdir -I~/inc B:=$$(file\ >>seen,B)two
-include answer.mk
$(info RESULT=[$(RESULT)] DIRS=[$(filter old/inc hdir/inc,$(.INCLUDE_DIRS))])
