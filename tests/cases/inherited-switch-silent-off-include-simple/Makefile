MAKEFLAGS := -s -I pool/include
$(file >seen,reader=[$(MAKEFLAGS)])
$(info reader=[$(MAKEFLAGS)])
include extra.mk
.PHONY: inspect
inspect:
	$(file >>seen,recipe=[$(MAKEFLAGS)])$(info recipe=[$(MAKEFLAGS)])printf "%s\n" parent
	+@make -C worker inspect
