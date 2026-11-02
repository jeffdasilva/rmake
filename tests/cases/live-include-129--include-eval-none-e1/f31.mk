ifeq (yes,yes)
$(file >>seen,b31)
$(eval -include f32.mk)
$(file >>seen,a31)
endif
