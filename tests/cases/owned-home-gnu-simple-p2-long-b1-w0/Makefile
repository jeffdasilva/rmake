$(file >seen,anchor)
unexport HOME A B RESULT
undefine HOME
RESULT := missing
GNUMAKEFLAGS = A:=$$(file\ >>seen,A)one HOME:=$$(file\ >>seen,SET)hdirÿ --include-dir=~/inc B:=$$(file\ >>seen,B)two
.PHONY: all
all:;@$(eval -include answer.mk)$(info RESULT=[$(RESULT)] DIRS=[$(filter oldÿ/inc hdirÿ/inc,$(.INCLUDE_DIRS))])true
