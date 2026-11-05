KEY := WRONG
$(foreach KEY,SLOT,$(eval chosen: $$(KEY) := from-loop)$(info loop=[$(KEY)]))
$(info global=[$(KEY)])
.PHONY: all chosen
all: chosen
chosen:;$(info slot=[$(SLOT)] origin=$(origin SLOT))@:
