$(file >seen,anchor)
unexport HOME READY RESULT A B
READY :=
RESULT := missing
HOME = $(file >>seen,HOME)$(if $(READY),./-,$(eval READY:=yes)~)
MAKEFLAGS += -Ikeep -I~
-include answer.mk
$(info RESULT=[$(RESULT)] DIRS=[$(filter keep ./-,$(.INCLUDE_DIRS))])
.PHONY: all
all:;
