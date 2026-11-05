$(foreach token,lexical,$(eval %.item: VALUE := $$(token)))
$(info outside=[$(token)])
.PHONY: all result.item
all: result.item
result.item:;$(info value=[$(VALUE)] origin=$(origin VALUE))@:
