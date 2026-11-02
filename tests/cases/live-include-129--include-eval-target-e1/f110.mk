ifeq (yes,yes)
$(file >>seen,b110)
$(eval -include f111.mk)
$(file >>seen,a110)
endif
