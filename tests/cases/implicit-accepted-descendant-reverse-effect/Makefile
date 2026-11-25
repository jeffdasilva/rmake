.SECONDEXPANSION:
%.out: %.tmp %.mid ; @cat effects
%.tmp: %.mid ; @:
%.mid: $$(eval SEEN += x) $$(file >>effects,searched) \
       $$(if $$(word 2,$$(SEEN)),$$(error descendant expanded twice)) %.in ; @:
