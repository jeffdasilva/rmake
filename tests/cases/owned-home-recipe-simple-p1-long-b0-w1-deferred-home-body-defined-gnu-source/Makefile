$(file >seen,anchor)
unexport HOME A B RESULT
HOME = $(MISSING_HOME)$(file >>seen,OLD)old
RESULT := missing
define ACTION
MAKEFLAGS = A:=$$(file\ >>seen,A)one --include-dir=~/inc HOME:=$$(file\ >>seen,SET)hdir B:=$$(file\ >>seen,B)two
endef
define FIRE
$(eval $(value ACTION))$(eval -include answer.mk)$(info RESULT=[$(RESULT)] DIRS=[$(filter old/inc hdir/inc,$(.INCLUDE_DIRS))])
endef
.PHONY: all
all:;@$(FIRE)true
