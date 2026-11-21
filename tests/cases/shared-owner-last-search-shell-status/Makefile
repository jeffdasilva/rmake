.SECONDEXPANSION:
%.out: %.mid %.mid ; @printf '%s\n' 'meta|$@|$(.SHELLSTATUS)|$+'
%.mid: $$(eval SEEN += x)$$(shell exit $$(words $$(SEEN))) %.in ; @printf '%s\n' 'meta|$@|$(.SHELLSTATUS)|$+'
