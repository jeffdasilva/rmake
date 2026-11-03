$(file >seen,anchor)
unexport HOME READY RESULT
READY :=
RESULT := missing
HOME := $(file >>seen,HOME)$(if $(READY),hdir,$(eval READY:=yes)~)
GNUMAKEFLAGS += -I~/inc
GNUMAKEFLAGS += -s
GNUMAKEFLAGS += -s
.PHONY: all
all:;@$(eval -include answer.mk)$(info RESULT=[$(RESULT)] DIRS=[$(filter ~/inc hdir/inc,$(.INCLUDE_DIRS))] RAW=[$(filter -I~/inc,$(MAKEFLAGS))])true
