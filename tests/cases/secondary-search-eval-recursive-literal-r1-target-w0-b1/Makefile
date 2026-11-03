$(file >seen,anchor)
unexport HOME READY RESULT
READY :=
RESULT := missing
HOME = $(file >>seen,HOME)~
define ACTION
MAKEFLAGS += -I~/incÿ
endef
$(eval $(value ACTION))$(eval -include answer.mk)$(info RESULT=[$(RESULT)] DIRS=[$(filter ~/incÿ hdir/incÿ,$(.INCLUDE_DIRS))] RAW=[$(filter -I~/incÿ,$(MAKEFLAGS))])
.PHONY: all
all:;
