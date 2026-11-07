.PHONY: build
build: ; @printf "ordinary\n"
-include catalog.cache
include catalog.dep
include catalog.dep
%.cache %.dep:
	@printf 'attempt %s\n' '$@'; exit 7
