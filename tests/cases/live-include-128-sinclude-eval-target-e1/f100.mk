ifeq (yes,yes)
$(file >>seen,b100)
$(eval sinclude f101.mk)
$(file >>seen,a100)
endif
