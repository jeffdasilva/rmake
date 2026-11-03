$(file >seen,anchor)
unexport HOME A B RESULT
HOME = $(file >>seen,OLD)oldÿ
RESULT := missing
define ACTION
MAKEFLAGS = -I~/inc A:=$$(file\ >>seen,A)one HOME:=$$(file\ >>seen,SET)hdirÿ B:=$$(file\ >>seen,B)two
endef
$(eval $(value ACTION))$(eval -include answer.mk)$(info RESULT=[$(RESULT)] DIRS=[$(filter oldÿ/inc hdirÿ/inc,$(.INCLUDE_DIRS))])
.PHONY: all
all:;
