$(file >seen,anchor)
unexport HOME A B RESULT
undefine HOME
RESULT := missing
GNUMAKEFLAGS = A:=$$(file\ >>seen,A)one HOME=$$(file\ >>seen,SET)hdir B:=$$(file\ >>seen,B)two -I~/inc
.PHONY: all
all:;@$(eval -include answer.mk)$(info RESULT=[$(RESULT)] DIRS=[$(filter old/inc hdir/inc,$(.INCLUDE_DIRS))])true
