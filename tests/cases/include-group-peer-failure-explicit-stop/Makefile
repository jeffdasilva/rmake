.PHONY: build
build: ; @printf "ordinary\n"
-include catalog.cache
include catalog.dep
catalog.cache catalog.dep &:
	@printf 'attempt %s\n' '$@'; exit 7
