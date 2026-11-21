define MUTATE
a.mid: extra
endef
.SECONDEXPANSION:
%.out: %.mid %.mid ; @printf '%s\n' 'recipe|$@|$<|$^|$+|$?|$|'
%.mid: $$(eval SEEN += x)$$(info search|$$@|$$(words $$(SEEN))) $$(if $$(word 2,$$(SEEN)),$$(eval $$(MUTATE))) %.in ; @printf '%s\n' 'recipe|$@|$<|$^|$+|$?|$|'
