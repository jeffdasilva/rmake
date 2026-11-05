SEED := global
chosen: SEED := own
chosen: VALUE := $(eval include extra.mk)$(SEED)/$(INCLUDED)/$(ADDED)
$(info reader=[$(SEED)] included=[$(INCLUDED)] added=[$(ADDED)])
.PHONY: all chosen
all: chosen
chosen:;$(info value=[$(VALUE)] flavor=$(flavor VALUE) origin=$(origin VALUE))@:
