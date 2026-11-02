ifeq (yes,yes)
$(file >>seen,b1)
sinclude f2.mk
$(file >>seen,a1)
endif
