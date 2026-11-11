MAKEFLAGS = -S
$(file >seen,reader=[$(MAKEFLAGS)])
$(info reader=[$(MAKEFLAGS)])
.PHONY: inspect broken after
inspect: broken after
broken:
	@$(file >>seen,recipe=[$(MAKEFLAGS)])$(info recipe=[$(MAKEFLAGS)])exit 7
after:
	@$(file >>seen,after=visited)printf "%s\n" continued
