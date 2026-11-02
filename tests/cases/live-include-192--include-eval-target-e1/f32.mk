ifeq (yes,yes)
$(file >>seen,b32)
$(eval -include f33.mk)
$(file >>seen,a32)
endif
