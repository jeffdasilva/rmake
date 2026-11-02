ifeq (yes,yes)
$(file >>seen,b2)
$(eval -include f3.mk)
$(file >>seen,a2)
endif
