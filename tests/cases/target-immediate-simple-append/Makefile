chosen: VALUE := before
chosen: SEED := own
chosen: VALUE += $(eval chosen: VALUE := nested)$(SEED)
.PHONY: all chosen
all: chosen
chosen:;$(info value=[$(VALUE)] flavor=$(flavor VALUE) origin=$(origin VALUE))@:
