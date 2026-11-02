ifeq (yes,yes)
$(file >>seen,b123)
$(eval sinclude f124.mk)
$(file >>seen,a123)
endif
