ifeq (yes,yes)
$(file >>seen,b1)
$(eval include f2.mk)
$(file >>seen,a1)
endif
