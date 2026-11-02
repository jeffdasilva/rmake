ifeq (yes,yes)
$(file >>seen,b4)
$(eval -include f5.mk)
$(file >>seen,a4)
endif
