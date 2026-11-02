ifeq (yes,yes)
$(file >>seen,b1)
-include f2.mk
$(file >>seen,a1)
endif
