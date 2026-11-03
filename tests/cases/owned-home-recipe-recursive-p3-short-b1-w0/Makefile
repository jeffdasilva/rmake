$(file >seen,anchor)
unexport HOME A B RESULT
HOME = $(file >>seen,OLD)oldÿ
RESULT := missing
define ACTION
MAKEFLAGS = A:=$$(file\ >>seen,A)one HOME=$$(file\ >>seen,SET)hdirÿ B:=$$(file\ >>seen,B)two -I~/inc
endef
define FIRE
$(eval $(value ACTION))$(eval -include answer.mk)$(info RESULT=[$(RESULT)] DIRS=[$(filter oldÿ/inc hdirÿ/inc,$(.INCLUDE_DIRS))])
endef
.PHONY: all
all:;@$(FIRE)true
