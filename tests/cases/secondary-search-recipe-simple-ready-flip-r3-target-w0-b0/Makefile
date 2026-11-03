$(file >seen,anchor)
unexport HOME READY RESULT
READY :=
RESULT := missing
HOME := $(file >>seen,HOME)$(if $(READY),hdir,$(eval READY:=yes)~)
define ACTION
MAKEFLAGS += -I~/inc
MAKEFLAGS += -s
MAKEFLAGS += -s
endef
define FIRE
$(eval $(value ACTION))$(eval -include answer.mk)$(info RESULT=[$(RESULT)] DIRS=[$(filter ~/inc hdir/inc,$(.INCLUDE_DIRS))] RAW=[$(filter -I~/inc,$(MAKEFLAGS))])
endef
.PHONY: all
all:;@$(FIRE)true
