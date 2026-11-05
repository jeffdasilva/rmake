SEED := before
chosen: SEED := own
chosen: VALUE :::= $(eval CREATED := effect)$(SEED)-$$X
SEED := after
$(info reader=[$(CREATED)])
.PHONY: all chosen
all: chosen
chosen:;$(info value=[$(VALUE)] flavor=$(flavor VALUE) origin=$(origin VALUE))@:
