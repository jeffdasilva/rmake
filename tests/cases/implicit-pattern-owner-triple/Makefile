.SECONDEXPANSION:
%.mid: V = fixed
%.mid: W ?= local
%.out: %.mid %.mid %.mid ; @:
%.mid: $$(eval SEEN += x) $$(shell exit $$(words $$(SEEN))) \
       $$(info search|$$@|$$(V)|$$(origin V)|$$(W)|$$(origin W)|$$(.SHELLSTATUS)) \
       $$(if $$(word 2,$$(SEEN)),,$$(eval W = global)) \
       %.in
	@printf "%s\n" "recipe|$@|$(V)|$(origin V)|$(W)|$(origin W)|$(.SHELLSTATUS)"
