$(file >seen,anchor)
unexport HOME READY RESULT A B
READY :=
RESULT := missing
HOME = $(file >>seen,HOME)$(if $(READY),$(eval HOME:=hdir)$(eval MAKEFLAGS = -Iextra)hdir,$(eval READY:=yes)~)
include trigger.mk
.PHONY: all
all:;
