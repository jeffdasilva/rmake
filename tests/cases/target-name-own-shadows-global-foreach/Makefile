KEY := WRONG
chosen: KEY := SLOT
$(foreach KEY,LOCAL,$(eval chosen: $$(KEY) := from-owner)$(info loop=[$(KEY)]))
.PHONY: all chosen
all: chosen
chosen:;$(info slot=[$(SLOT)] origin=$(origin SLOT))@:
