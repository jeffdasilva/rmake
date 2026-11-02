ifeq (yes,yes)
$(file >>seen,b89)
$(eval -include f90.mk)
$(file >>seen,a89)
endif
