ifeq (yes,yes)
$(file >>seen,b3)
$(eval sinclude f4.mk)
$(file >>seen,a3)
endif
