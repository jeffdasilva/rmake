.SECONDEXPANSION:
%.out: %.tmp absent ; @printf '%s\n' 'outer|$@'
%.tmp: %.mid ; @printf '%s\n' 'bridge|$@'
%.mid: $$(eval SEEN += x) $$(file >>effects,search-$$(words $$(SEEN))) \
       $$(word $$(words $$(SEEN)),a.in a.gate)
	@printf '%s\n' 'child|$@|$+'
