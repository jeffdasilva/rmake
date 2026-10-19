x.out:
%.out: %.mid;@echo final;cp $< $@
%.mid: %.src;@echo middle;cp $< $@
.INTERMEDIATE: x.mid
x.mid: x.src;@echo explicit;cp $< $@
