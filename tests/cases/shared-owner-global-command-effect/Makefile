CMD := before
.SECONDEXPANSION:
%.out: %.mid %.mid ; @printf '%s\n' 'command|$@|$(CMD)|$+'
%.mid: $$(eval SEEN += x)$$(if $$(word 2,$$(SEEN)),$$(eval CMD = after)) %.in ; @printf '%s\n' 'command|$@|$(CMD)|$+'
