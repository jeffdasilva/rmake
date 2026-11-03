$(file >seen,anchor)
unexport HOME READY RESULT A B
READY :=
RESULT := missing
HOME = $(file >>seen,HOME)$(if $(READY),$(eval HOME:=hdir)$(eval MAKEFLAGS = -Iextra)hdir,$(eval READY:=yes)~)
define ACTION
MAKEFLAGS += -I~/inc A:=$$(file\ >>seen,A)one B:=$$(file\ >>seen,B)two
endef
define FIRE
$(eval $(value ACTION))$(eval -include answer.mk)$(info RESULT=[$(RESULT)] DIRS=[$(filter hdir/inc extra,$(.INCLUDE_DIRS))] A=[$(A)] B=[$(B)])
endef
.PHONY: all
.SECONDEXPANSION:
all: $$(FIRE);
