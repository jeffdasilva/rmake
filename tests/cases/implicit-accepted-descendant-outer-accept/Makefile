.EXTRA_PREREQS = guard hidden
.SECONDEXPANSION:
%.out: %.mid %.tmp ; @printf '%s\n' 'outer|$@|$+'
%.tmp: %.mid ; @printf '%s\n' 'bridge|$@|$+'
%.mid: $$(eval SEEN += x) $$(file >>effects,search-$$(words $$(SEEN))) \
       $$(shell exit $$(words $$(SEEN))) \
       $$(eval MAKEFLAGS += $$(word $$(words $$(SEEN)),--warn-undefined-variables --silent --no-keep-going)) \
       $$(word $$(words $$(SEEN)),a.in a.gate extra)
	@test -f hidden
	@printf '%s\n' 'child|$@|$<|$^|$+|$||$(.SHELLSTATUS)|$(MAKEFLAGS)'
hidden:
	@printf '%s\n' hidden > $@
