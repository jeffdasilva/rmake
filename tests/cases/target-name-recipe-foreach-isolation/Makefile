KEY := SLOT
.PHONY: all caller chosen
all: caller chosen
caller: KEY := RECIPE
caller:;$(foreach KEY,LOOP,$(eval chosen: $$(KEY) := from-global)$(info loop=[$(KEY)]))$(info caller=[$(KEY)])@:
chosen:;$(info slot=[$(SLOT)] origin=$(origin SLOT))@:
