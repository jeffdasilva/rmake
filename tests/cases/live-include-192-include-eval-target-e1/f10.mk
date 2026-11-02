ifeq (yes,yes)
$(file >>seen,b10)
$(eval include f11.mk)
$(file >>seen,a10)
endif
