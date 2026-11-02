ifeq (yes,yes)
$(file >>seen,b0)
include f1.mk
$(file >>seen,a0)
endif
