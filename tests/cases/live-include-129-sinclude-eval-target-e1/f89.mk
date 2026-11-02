ifeq (yes,yes)
$(file >>seen,b89)
$(eval sinclude f90.mk)
$(file >>seen,a89)
endif
