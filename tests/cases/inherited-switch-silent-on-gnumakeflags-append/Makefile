MAKEFLAGS += --no-silent
$(file >seen,reader=[$(MAKEFLAGS)])
$(info reader=[$(MAKEFLAGS)])
.PHONY: inspect
inspect:
	$(file >>seen,recipe=[$(MAKEFLAGS)])$(info recipe=[$(MAKEFLAGS)])printf "%s\n" parent
	+@make -C worker inspect
