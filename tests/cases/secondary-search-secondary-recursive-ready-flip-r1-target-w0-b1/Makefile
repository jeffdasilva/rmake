$(file >seen,anchor)
unexport HOME READY RESULT
READY :=
RESULT := missing
HOME = $(file >>seen,HOME)$(if $(READY),hdir,$(eval READY:=yes)~)
define ACTION
MAKEFLAGS += -I~/incÿ
endef
define FIRE
$(eval $(value ACTION))$(eval -include answer.mk)$(info RESULT=[$(RESULT)] DIRS=[$(filter ~/incÿ hdir/incÿ,$(.INCLUDE_DIRS))] RAW=[$(filter -I~/incÿ,$(MAKEFLAGS))])
endef
.PHONY: all
.SECONDEXPANSION:
all: $$(FIRE);
