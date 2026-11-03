$(file >seen,anchor)
unexport HOME A B RESULT
HOME = $(file >>seen,OLD)oldÿ
RESULT := missing
define ACTION
MAKEFLAGS = --include-dir=~/inc A:=$$(file\ >>seen,A)one HOME=$$(file\ >>seen,SET)hdirÿ B:=$$(file\ >>seen,B)two
endef
define FIRE
$(eval $(value ACTION))$(eval -include answer.mk)$(info RESULT=[$(RESULT)] DIRS=[$(filter oldÿ/inc hdirÿ/inc,$(.INCLUDE_DIRS))])
endef
.PHONY: all
.SECONDEXPANSION:
all: $$(FIRE) ;
