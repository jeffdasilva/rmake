.PHONY: build
build: ; @printf "ordinary\n"
-include catalog.cache
include catalog.dep
include tail.dep
%.cache %.dep:
	@printf 'attempt %s\n' '$@'; exit 7
tail.dep:
	@printf 'tail attempted\n'; exit 8
