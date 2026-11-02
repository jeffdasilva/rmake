ifeq (yes,yes)
$(file >>seen,b0)
$(eval sinclude f1.mk)
$(file >>seen,a0)
endif
