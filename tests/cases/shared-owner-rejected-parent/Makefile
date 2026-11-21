.SECONDEXPANSION:
%.out: %.mid %.absent ; @printf '%s\n' 'recipe|$@|$<|$^|$+|$?|$|'
%.out: %.tmp ; @printf '%s\n' 'recipe|$@|$<|$^|$+|$?|$|'
%.tmp: %.mid ; @printf '%s\n' 'recipe|$@|$<|$^|$+|$?|$|'
%.mid: $$(eval SEEN += x)$$(info search|$$@|$$(words $$(SEEN))) $$(word $$(words $$(SEEN)),a.in a.gate extra) ; @printf '%s\n' 'recipe|$@|$<|$^|$+|$?|$|'
