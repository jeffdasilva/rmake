$(foreach token,from-include,$(eval %.item: VALUE := $$(eval include extra.mk)$$(INCLUDED)))
$(info reader=[$(INCLUDED)] outside=[$(token)])
.PHONY: all result.item
all: result.item
result.item:;$(info value=[$(VALUE)] origin=$(origin VALUE))@:
