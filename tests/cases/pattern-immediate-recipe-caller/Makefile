.PHONY: all setup result.item
all: setup result.item
setup: LOCAL := target-caller
setup:;$(eval %.item: VALUE := $$(LOCAL))@:
result.item:;$(info value=[$(VALUE)] origin=$(origin VALUE))@:
