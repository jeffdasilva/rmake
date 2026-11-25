.SECONDEXPANSION:
%.out: %.mid %.tmp ; @printf '%s\n' 'outer|$@|$+'
%.out: %.mid ; @printf '%s\n' 'fallback|$@|$+'
%.tmp: %.mid absent ; @printf '%s\n' 'bridge|$@|$+'
%.mid: $$(eval SEEN += x) $$(file >>effects,search-$$(words $$(SEEN))) \
       $$(word $$(words $$(SEEN)),a.in a.gate extra)
	@printf '%s\n' 'child|$@|$+'
