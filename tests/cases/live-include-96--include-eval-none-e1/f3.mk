ifeq (yes,yes)
$(file >>seen,b3)
$(eval -include f4.mk)
$(file >>seen,a3)
endif
