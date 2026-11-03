$(file >seen,anchor)
unexport HOME A B RESULT
undefine HOME
RESULT := missing
GNUMAKEFLAGS = A:=$$(file\ >>seen,A)one --include-dir=~/inc HOME=$$(file\ >>seen,SET)hdir B:=$$(file\ >>seen,B)two
.PHONY: all
all:;@$(eval -include answer.mk)$(info RESULT=[$(RESULT)] DIRS=[$(filter old/inc hdir/inc,$(.INCLUDE_DIRS))])true
