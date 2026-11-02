ifeq (yes,yes)
$(file >>seen,b2)
sinclude f3.mk
$(file >>seen,a2)
endif
