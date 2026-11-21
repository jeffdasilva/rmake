.SECONDEXPANSION:
%.out: %.mid %.mid ; @printf '%s\n' 'recipe|$@|$<|$^|$+|$?|$|'
%.mid: $$(eval SEEN += x)$$(info search|$$@|$$(words $$(SEEN))) $$(word $$(words $$(SEEN)),a.in a.gate extra) ; @printf '%s\n' 'recipe|$@|$<|$^|$+|$?|$|'
