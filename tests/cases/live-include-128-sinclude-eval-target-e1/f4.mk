ifeq (yes,yes)
$(file >>seen,b4)
$(eval sinclude f5.mk)
$(file >>seen,a4)
endif
