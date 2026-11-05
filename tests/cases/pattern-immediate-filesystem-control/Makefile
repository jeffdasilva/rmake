GLOBAL := direct
%.item: VALUE := $(file >>seen,rhs)$(GLOBAL)
.PHONY: all result.item
all: result.item
result.item:;$(info value=[$(VALUE)] origin=$(origin VALUE))@:
