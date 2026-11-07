.PHONY: build
build: ; @printf "ordinary\n"
include catalog.dep
-include catalog.cache
%.cache %.dep:
	@printf 'attempt %s\n' '$@'; exit 7
