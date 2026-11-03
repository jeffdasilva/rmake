$(file >seen,anchor)
unexport HOME READY RESULT
READY :=
RESULT := missing
HOME = $(file >>seen,HOME)$(if $(READY),hdir,$(eval READY:=yes)~)
include trigger.mk
.PHONY: all
all:;
