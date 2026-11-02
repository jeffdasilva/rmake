ifeq (yes,yes)
$(file >>seen,b99)
$(eval -include f100.mk)
$(file >>seen,a99)
endif
