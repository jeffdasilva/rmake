$(file >seen,anchor)
unexport HOME READY RESULT A B
READY :=
RESULT := missing
HOME = $(file >>seen,HOME)$(if $(READY),$(error secondary-stop),$(eval READY:=yes)~)
MAKEFLAGS += -I~/inc A:=$$(file\ >>seen,A)done
$(file >>seen,FORBIDDEN)
.PHONY: all
all:;
