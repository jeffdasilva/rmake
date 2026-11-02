ifeq (yes,yes)
$(file >>seen,b10)
$(eval sinclude f11.mk)
$(file >>seen,a10)
endif
