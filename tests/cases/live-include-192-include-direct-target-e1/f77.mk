ifeq (yes,yes)
$(file >>seen,b77)
include f78.mk
$(file >>seen,a77)
endif
