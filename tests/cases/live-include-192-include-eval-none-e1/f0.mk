ifeq (yes,yes)
$(file >>seen,b0)
$(eval include f1.mk)
$(file >>seen,a0)
endif
