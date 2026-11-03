$(file >seen,anchor)
unexport HOME READY RESULT
READY :=
RESULT := missing
HOME := $(MISSING_HOME)$(file >>seen,HOME)$(if $(READY),hdir,$(eval READY:=yes)~)
MAKEFLAGS += -I~/incÿ
-include answer.mk
$(info RESULT=[$(RESULT)] DIRS=[$(filter ~/incÿ hdir/incÿ,$(.INCLUDE_DIRS))] RAW=[$(filter -I~/incÿ,$(MAKEFLAGS))])
.PHONY: all
all:;
