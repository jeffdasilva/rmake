$(file >seen,anchor)
unexport HOME READY RESULT A B
READY :=
RESULT := missing
HOME = $(file >>seen,HOME)$(if $(READY),$(eval HOME:=hdir)$(eval MAKEFLAGS = -- -x)hdir,$(eval READY:=yes)~)
MAKEFLAGS += -I~/inc A:=$$(file\ >>seen,A)one B:=$$(file\ >>seen,B)two
-include answer.mk
$(info RESULT=[$(RESULT)] DIRS=[$(filter hdir/inc extra,$(.INCLUDE_DIRS))] A=[$(A)] B=[$(B)])
.PHONY: all
all:;
