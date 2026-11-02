ifeq (yes,yes)
$(file >>seen,b99)
$(eval sinclude f100.mk)
$(file >>seen,a99)
endif
