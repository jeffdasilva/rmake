ifeq (yes,yes)
$(file >>seen,b41)
$(eval -include f42.mk)
$(file >>seen,a41)
endif
