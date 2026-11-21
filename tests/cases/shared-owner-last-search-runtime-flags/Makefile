.SECONDEXPANSION:
%.out: %.mid %.mid ; @printf '%s\n' 'meta|$@|$(MAKEFLAGS)|$+'
%.mid: $$(eval MAKEFLAGS += --warn-undefined-variables) %.in ; @printf '%s\n' 'meta|$@|$(MAKEFLAGS)|$+'
